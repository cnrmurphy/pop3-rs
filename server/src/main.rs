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

pub type IOResult<T> = std::io::Result<T>;

const PORT: u16 = 110;

pub struct Session {
    state: SessionState,
    user: Option<String>,
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
    let session_manager = Arc::new(SessionManager::new());
    let listener = TcpListener::bind("127.0.0.1:1110").await.unwrap();

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let session_manager = Arc::clone(&session_manager);
        println!("new connection");
        tokio::spawn(async move {
            process(stream, session_manager).await;
        });
    }
}

async fn process(mut stream: TcpStream, session_manager: Arc<SessionManager>) -> IOResult<()> {
    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);
    println!("writing greeting");
    let greeting = StatusIndicator::Ok("POP3 server ready".to_string());
    writer.write(greeting.to_string().as_bytes()).await?;
    writer.flush().await?;

    let mut session = Session {
        state: SessionState::Authorization,
        user: None,
        mailbox_lock: None,
    };
    let mut line = String::new();

    loop {
        line.clear();
        reader.read_line(&mut line).await?;
        match Command::parse(line.trim()) {
            Ok(cmd) => {
                let should_quit = matches!(cmd, Command::Quit);
                let resp = handle_command(cmd, &mut session, &session_manager);
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
) -> StatusIndicator {
    match cmd {
        Command::Apop => StatusIndicator::Ok("APOP".to_string()),
        Command::User(username) => {
            if !matches!(session.state, SessionState::Authorization) {
                return StatusIndicator::Err("Session not in Authorization state ".to_string());
            }
            session.user = Some(username);
            StatusIndicator::Ok("User accepted".to_string())
        }
        Command::Pass(password) => {
            if !matches!(session.state, SessionState::Authorization) {
                return StatusIndicator::Err("Session not in Authorization state ".to_string());
            }
            if session.user.is_none() {
                return StatusIndicator::Err("No username set - send USER first".to_string());
            }
            let username = session.user.as_ref().unwrap();
            match session_manager.try_lock_mailbox(username, Arc::clone(session_manager)) {
                Ok(lock) => {
                    session.state = SessionState::Transaction;
                    session.mailbox_lock = Some(lock);
                    StatusIndicator::Ok("Pass accepted".to_string())
                }
                Err(_) => StatusIndicator::Err("Mailbox already in use".to_string()),
            }
        }
        Command::Noop => StatusIndicator::Ok("NOOP".to_string()),
        Command::Quit => {
            // Only when the session state is in the TRANSACTION state does the state need to be
            // set to the UPDATE state when the QUIT command is issued!
            if matches!(session.state, SessionState::Transaction) {
                session.state = SessionState::Update;
            }
            // TODO: release resources
            StatusIndicator::Ok("Bye!".to_string())
        }
    }
}
