pub mod auth;
pub mod maildir;
pub mod protocol;

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use protocol::{Command, SessionState, StatusIndicator};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};

use crate::{auth::AuthStore, maildir::MailDir};

pub type IOResult<T> = std::io::Result<T>;

const PORT: u16 = 110;

pub struct Session {
    state: SessionState,
    mailbox_lock: Option<MailboxLock>,
}

pub struct SessionManager {
    locked_mailboxes: Mutex<HashSet<String>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            locked_mailboxes: Mutex::new(HashSet::new()),
        }
    }

    pub fn try_lock_mailbox(
        &self,
        username: &str,
        manager_arc: Arc<SessionManager>,
    ) -> Result<MailboxLock, &'static str> {
        let mut lock = self.locked_mailboxes.lock().unwrap();
        if lock.contains(username) {
            Err("Mailbox already locked")
        } else {
            lock.insert(username.to_string());
            Ok(MailboxLock {
                username: username.to_string(),
                manager: manager_arc,
            })
        }
    }

    fn unlock_mailbox(&self, username: &str) {
        let mut lock = self.locked_mailboxes.lock().unwrap();
        lock.remove(username);
    }
}

pub struct MailboxLock {
    username: String,
    manager: Arc<SessionManager>,
}

impl Drop for MailboxLock {
    fn drop(&mut self) {
        self.manager.unlock_mailbox(&self.username);
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let db = sled::open("my_db").unwrap();

    // TODO: we might want to implement an independent helper cli at some point
    if args.len() >= 2 && args[1] == "add-user" {
        if args.len() != 4 {
            eprintln!("Usage: {} add-user <username> <password>", args[0]);
            std::process::exit(1);
        }

        let auth_store = auth::AuthStore::new(db);

        match auth_store.create_user(&args[2], &args[3]) {
            Ok(true) => {
                maildir::init_user_mailbox(&args[2]).unwrap();
                println!("User '{}' created successfully", args[2])
            }
            Ok(false) => println!("User '{}' already exists", args[2]),
            Err(e) => eprintln!("Error creating user: {}", e),
        }
        return;
    }

    let auth = Arc::new(auth::AuthStore::new(db));
    let session_manager = Arc::new(SessionManager::new());
    let listener = TcpListener::bind("127.0.0.1:1110").await.unwrap();

    println!("Mail server listening on 127.0.0.1:1110");
    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let session_manager = Arc::clone(&session_manager);
        let auth_store = Arc::clone(&auth);
        println!("new connection");
        tokio::spawn(async move {
            let _ = process(stream, session_manager, auth_store).await;
        });
    }
}

async fn process(
    mut stream: TcpStream,
    session_manager: Arc<SessionManager>,
    auth_store: Arc<AuthStore>,
) -> IOResult<()> {
    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);
    println!("writing greeting");
    let greeting = StatusIndicator::Ok("POP3 server ready".to_string());
    writer.write(greeting.to_string().as_bytes()).await?;
    writer.flush().await?;

    let mut session = Session {
        state: SessionState::Authorization,
        mailbox_lock: None,
    };
    let mut line = String::new();

    loop {
        line.clear();
        reader.read_line(&mut line).await?;
        match Command::parse(line.trim()) {
            Ok(cmd) => {
                let should_quit = matches!(cmd, Command::Quit);
                let resp = handle_command(cmd, &mut session, &session_manager, &auth_store);
                send_response(&mut writer, resp).await?;
                if should_quit {
                    return Ok(());
                }
            }
            Err(e) => {
                println!("{}", e.to_string());
                writer.write(e.to_string().as_bytes()).await?;
                writer.flush().await?;
            }
        }
    }
}

async fn send_response(
    writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>,
    resp: StatusIndicator,
) -> IOResult<()> {
    writer.write(resp.to_string().as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

fn handle_command(
    cmd: Command,
    session: &mut Session,
    session_manager: &Arc<SessionManager>,
    auth_store: &Arc<AuthStore>,
) -> StatusIndicator {
    match cmd {
        Command::Apop => StatusIndicator::Ok("APOP".to_string()),
        Command::User(username) => {
            if !matches!(session.state, SessionState::Authorization) {
                return StatusIndicator::Err("Session not in Authorization state ".to_string());
            }
            session.state = SessionState::AuthorizationWithUser(username.to_string());
            StatusIndicator::Ok("User accepted".to_string())
        }
        Command::Pass(password) => match &session.state {
            SessionState::AuthorizationWithUser(username) => {
                match auth_store.login(username, &password) {
                    Ok(success) => {
                        if !success {
                            return StatusIndicator::Err(
                                "Username or password are incorrect".to_string(),
                            );
                        }
                        match session_manager
                            .try_lock_mailbox(username, Arc::clone(session_manager))
                        {
                            Ok(lock) => {
                                session.state = SessionState::Transaction(username.to_string());
                                session.mailbox_lock = Some(lock);
                                StatusIndicator::Ok("Pass accepted".to_string())
                            }
                            Err(_) => StatusIndicator::Err("Mailbox already in use".to_string()),
                        }
                    }
                    Err(e) => {
                        println!("{}", e);
                        StatusIndicator::Err("Username or password are incorrect".to_string())
                    }
                }
            }
            _ => StatusIndicator::Err("No username set - send USER first".to_string()),
        },
        Command::List => match &session.state {
            SessionState::Transaction(username) => {
                let messages = MailDir::list_messages(username);
                println!("{} messages", messages.len());
                StatusIndicator::Ok(format!("{} messages", messages.len()))
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Noop => StatusIndicator::Ok("NOOP".to_string()),
        Command::Quit => {
            // Only when the session state is in the TRANSACTION state does the state need to be
            // set to the UPDATE state when the QUIT command is issued!
            match &session.state {
                SessionState::Transaction(username) => {
                    session.state = SessionState::Update(username.to_string());
                    StatusIndicator::Ok("Bye!".to_string())
                }
                _ => StatusIndicator::Ok("Bye!".to_string()),
            }
        }
    }
}
