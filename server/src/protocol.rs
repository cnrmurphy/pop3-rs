pub enum StatusIndicator {
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

#[derive(Debug)]
pub enum SessionState {
    Authorization,
    AuthorizationWithUser(String),
    Update(String),
    Transaction(String),
}

pub enum Command {
    Apop,
    Noop,
    Pass(String),
    Quit,
    User(String),
    List,
    Retr(u64),
    Dele(u64),
    Rset,
}

impl Command {
    pub fn parse(input: &str) -> Result<Command, StatusIndicator> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts.first().map(|s| s.to_uppercase()).as_deref() {
            Some("USER") => match parts.get(1) {
                Some(username) => Ok(Command::User(username.to_string())),
                None => Err(StatusIndicator::Err("USER requires username".to_string())),
            },
            Some("PASS") => match parts.get(1) {
                Some(password) => Ok(Command::Pass(password.to_string())),
                None => Err(StatusIndicator::Err("PASS requires password".to_string())),
            },
            Some("RETR") => match parts.get(1) {
                Some(message_id) => match message_id.parse::<u64>() {
                    Ok(id) => Ok(Command::Retr(id)),
                    Err(e) => Err(StatusIndicator::Err(
                        format!("error parsing ID: {}", e.to_string()).to_string(),
                    )),
                },
                None => Err(StatusIndicator::Err("RETR requires mail id".to_string())),
            },
            Some("DELE") => match parts.get(1) {
                Some(message_id) => match message_id.parse::<u64>() {
                    Ok(id) => Ok(Command::Dele(id)),
                    Err(e) => Err(StatusIndicator::Err(
                        format!("error parsing ID: {}", e.to_string()).to_string(),
                    )),
                },
                None => Err(StatusIndicator::Err("DELE requires mail id".to_string())),
            },
            Some("RSET") => Ok(Command::Rset),
            Some("APOP") => Ok(Command::Apop),
            Some("NOOP") => Ok(Command::Noop),
            Some("LIST") => Ok(Command::List),
            Some("QUIT") => Ok(Command::Quit),
            _ => Err(StatusIndicator::Err("Unknown command".to_string())),
        }
    }
}
