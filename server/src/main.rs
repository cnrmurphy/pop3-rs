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
    Pass,
    Quit,
    User,
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
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }
    }

    Ok(())
}
