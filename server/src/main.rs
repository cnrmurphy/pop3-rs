use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};

pub type IOResult<T> = std::io::Result<T>;

const PORT: u16 = 110;

enum StatusIndicator {
    Ok(String),
    Err(String),
}

impl std::fmt::Display for StatusIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusIndicator::Ok(msg) => write!(f, "+OK {}\r\n", msg),
            StatusIndicator::Err(msg) => write!(f, "-ERR {}\r\n", msg),
        }
    }
}

enum SessionState {
    Authorization,
    Update,
    Transaction,
}

enum Command {
    Apop,
    Noop,
    Pass(String),
    Quit,
    User(String),
}

impl Command {
    fn parse(input: &str) -> Result<Command, StatusIndicator> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts.first().map(|s| s.to_uppercase()).as_deref() {
            Some("USER") => match parts.get(1) {
                Some(username) => Ok(Command::User(username.to_string())),
                None => Err(StatusIndicator::Err("USER requires username".to_string())),
            },
            Some("PASS") => match parts.get(1) {
                Some(password) => Ok(Command::Pass(password.to_string())),
                None => Err(StatusIndicator::Err("USER requires username".to_string())),
            },
            Some("APOP") => Ok(Command::Apop),
            Some("NOOP") => Ok(Command::Noop),
            Some("QUIT") => Ok(Command::Quit),
            _ => Err(StatusIndicator::Err("Unknown command".to_string())),
        }
    }
}

pub struct Session {
    state: SessionState,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:1110").await.unwrap();

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        println!("new connection");
        tokio::spawn(async move {
            process(stream).await;
        });
    }
}

async fn process(mut stream: TcpStream) -> IOResult<()> {
    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);
    println!("writing greeting");
    let greeting = StatusIndicator::Ok("POP3 server ready".to_string());
    writer.write(greeting.to_string().as_bytes()).await?;
    writer.flush().await?;

    let mut session = Session {
        state: SessionState::Authorization,
    };
    let mut line = String::new();

    loop {
        line.clear();
        reader.read_line(&mut line).await?;
        match Command::parse(line.trim()) {
            Ok(cmd) => {
                match cmd {
                    Command::Apop => {
                        let resp = StatusIndicator::Ok("APOP".to_string());
                        writer.write(resp.to_string().as_bytes()).await?;
                        writer.flush().await?;
                    }
                    Command::User(username) => {
                        if !matches!(session.state, SessionState::Authorization) {
                            let resp = StatusIndicator::Err(
                                "Session not in Authorization state ".to_string(),
                            );
                            writer.write(resp.to_string().as_bytes()).await?;
                            writer.flush().await?;
                        }
                        let resp = StatusIndicator::Ok("User accepted".to_string());
                        writer.write(resp.to_string().as_bytes()).await?;
                        writer.flush().await?;
                    }
                    Command::Pass(password) => {
                        if !matches!(session.state, SessionState::Authorization) {
                            let resp = StatusIndicator::Err(
                                "Session not in Authorization state ".to_string(),
                            );
                            writer.write(resp.to_string().as_bytes()).await?;
                            writer.flush().await?;
                        }
                        let resp = StatusIndicator::Ok("Password accepted".to_string());
                        writer.write(resp.to_string().as_bytes()).await?;
                        writer.flush().await?;
                    }
                    Command::Noop => {
                        let resp = StatusIndicator::Ok("NOOP".to_string());
                        writer.write(resp.to_string().as_bytes()).await?;
                        writer.flush().await?;
                    }
                    Command::Quit => {
                        let resp = StatusIndicator::Ok("Bye!".to_string());
                        // TODO: clear session state
                        writer.write(resp.to_string().as_bytes()).await?;
                        writer.flush().await?;
                        return Ok(());
                    }
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
