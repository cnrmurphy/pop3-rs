pub mod protocol;

use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use protocol::{Command, SessionState, StatusIndicator};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};

use auth::AuthStore;
use maildir::{MailDir, MailEntry};

pub type IOResult<T> = std::io::Result<T>;

struct MailboxCache {
    messages: BTreeMap<u64, MailEntry>,
    total_octets: u64,
}

impl MailboxCache {
    fn new(maildir: &MailDir) -> Self {
        let entries = maildir.list_messages();
        let mut messages = BTreeMap::new();
        let mut total_octets = 0;
        for (i, entry) in entries.into_iter().enumerate() {
            let id = (i as u64) + 1;
            total_octets += entry.size;
            messages.insert(id, entry);
        }
        Self {
            messages,
            total_octets,
        }
    }
}

pub struct Session {
    state: SessionState,
    mailbox_lock: Option<MailboxLock>,
    cache: Option<MailboxCache>,
    messages_marked_for_deletion: HashSet<u64>,
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

    if args.len() >= 2 && args[1] == "add-user" {
        if args.len() != 4 {
            eprintln!("Usage: {} add-user <username> <password>", args[0]);
            std::process::exit(1);
        }

        let auth_store = AuthStore::new(db);

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

    let auth = Arc::new(AuthStore::new(db));
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
        cache: None,
        messages_marked_for_deletion: HashSet::new(),
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
            if !matches!(
                session.state,
                SessionState::Authorization | SessionState::AuthorizationWithUser(_)
            ) {
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
                            Ok(lock) => match MailDir::new(&username) {
                                Ok(maildir) => {
                                    let cache = MailboxCache::new(&maildir);
                                    session.mailbox_lock = Some(lock);
                                    session.cache = Some(cache);
                                    session.state =
                                        SessionState::Transaction(username.to_string());
                                    StatusIndicator::Ok("Pass accepted".to_string())
                                }
                                Err(e) => StatusIndicator::Err(format!(
                                    "Failed to access mailbox: {}",
                                    e
                                )),
                            },
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
            SessionState::Transaction(_) => {
                let cache = session.cache.as_ref().unwrap();
                let mut resp = String::new();
                let mut total = 0;
                for (id, entry) in &cache.messages {
                    if session.messages_marked_for_deletion.contains(id) {
                        continue;
                    }
                    resp.push_str(&format!("{} {}\r\n", id, entry.size));
                    total += 1;
                }
                let resp = format!(
                    "{} messages ({} octets)\r\n{}.",
                    total, cache.total_octets, resp
                );
                StatusIndicator::Ok(resp)
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Retr(message_id) => match &session.state {
            SessionState::Transaction(_) => {
                if session.messages_marked_for_deletion.contains(&message_id) {
                    return StatusIndicator::Err(format!(
                        "message {} already deleted",
                        &message_id
                    ));
                }
                let cache = session.cache.as_ref().unwrap();
                match cache.messages.get(&message_id) {
                    Some(entry) => match entry.read() {
                        Ok(msg) => StatusIndicator::Ok(format!("{}.", msg)),
                        Err(e) => StatusIndicator::Err(format!("{}", e)),
                    },
                    None => StatusIndicator::Err("no such message".to_string()),
                }
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Dele(message_id) => match &session.state {
            SessionState::Transaction(_) => {
                let cache = session.cache.as_ref().unwrap();
                if !cache.messages.contains_key(&message_id) {
                    return StatusIndicator::Err("message does not exist".to_string());
                }
                if session.messages_marked_for_deletion.insert(message_id) {
                    return StatusIndicator::Ok(format!("message {} deleted", message_id));
                }
                StatusIndicator::Err(format!("message {} already deleted", &message_id))
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Rset => match &session.state {
            SessionState::Transaction(_) => {
                let cache = session.cache.as_ref().unwrap();
                session.messages_marked_for_deletion.clear();
                let resp = format!(
                    "{} messages ({} octets)",
                    cache.messages.len(),
                    cache.total_octets,
                );
                StatusIndicator::Ok(resp)
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Uidl(msg_id) => match &session.state {
            SessionState::Transaction(_) => {
                let cache = session.cache.as_ref().unwrap();
                match msg_id {
                    Some(id) => {
                        if session.messages_marked_for_deletion.contains(&id) {
                            return StatusIndicator::Err(format!(
                                "message {} already deleted",
                                id
                            ));
                        }
                        match cache.messages.get(&id) {
                            Some(entry) => {
                                StatusIndicator::Ok(format!("{} {}", id, entry.uidl))
                            }
                            None => StatusIndicator::Err("no such message".to_string()),
                        }
                    }
                    None => {
                        let mut resp = String::new();
                        for (id, entry) in &cache.messages {
                            if session.messages_marked_for_deletion.contains(id) {
                                continue;
                            }
                            resp.push_str(&format!("{} {}\r\n", id, entry.uidl));
                        }
                        StatusIndicator::Ok(format!("\r\n{}.", resp))
                    }
                }
            }
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Noop => match session.state {
            SessionState::Transaction(_) => StatusIndicator::Ok("NOOP".to_string()),
            _ => StatusIndicator::Err("Session not in Transaction state ".to_string()),
        },
        Command::Quit => {
            match &session.state {
                SessionState::Transaction(username) => {
                    session.state = SessionState::Update(username.to_string());
                    if !session.messages_marked_for_deletion.is_empty() {
                        let cache = session.cache.as_ref().unwrap();
                        let mut failed_to_delete = 0;
                        for id in &session.messages_marked_for_deletion {
                            if let Some(entry) = cache.messages.get(id) {
                                if let Err(e) = entry.delete() {
                                    println!("{}", e);
                                    failed_to_delete += 1;
                                }
                            }
                        }
                        if failed_to_delete > 0 {
                            return StatusIndicator::Err(
                                "some deleted messages not removed".to_string(),
                            );
                        }
                    }
                    StatusIndicator::Ok("Bye!".to_string())
                }
                _ => StatusIndicator::Ok("Bye!".to_string()),
            }
        }
    }
}
