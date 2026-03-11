use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

const MAILDIR_BASE: &str = "Maildir";

pub fn init_user_mailbox(username: &str) -> io::Result<()> {
    let mailbox = format!("{MAILDIR_BASE}/{username}");
    fs::create_dir_all(format!("{mailbox}/cur"))?;
    fs::create_dir_all(format!("{mailbox}/new"))?;
    fs::create_dir_all(format!("{mailbox}/tmp"))?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum MailDirError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("mail entry not found: {0}")]
    MailEntryNotFound(String),
}

pub struct MailDir {
    mailbox_new: PathBuf,
    mailbox_cur: PathBuf,
}

pub struct MailEntry {
    pub path: PathBuf,
    pub size: u64,
    pub filename: String,
    pub uidl: String,
}

impl MailEntry {
    pub fn read(&self) -> Result<String, MailDirError> {
        fs::read_to_string(&self.path).map_err(MailDirError::IoError)
    }

    pub fn delete(&self) -> Result<(), MailDirError> {
        fs::remove_file(&self.path).map_err(MailDirError::IoError)
    }
}

impl MailDir {
    pub fn new(username: &str) -> io::Result<Self> {
        let mailbox_new = PathBuf::from(format!("{MAILDIR_BASE}/{username}/new"));
        let mailbox_cur = PathBuf::from(format!("{MAILDIR_BASE}/{username}/cur"));
        Ok(Self {
            mailbox_new,
            mailbox_cur,
        })
    }

    pub fn list_messages(&self) -> Vec<MailEntry> {
        let mut entries = Vec::new();
        scan_dir(&self.mailbox_new, &mut entries);
        scan_dir(&self.mailbox_cur, &mut entries);
        entries
    }
}

fn scan_dir(dir: &Path, entries: &mut Vec<MailEntry>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in read_dir {
        let e = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = e.path();
        if path.is_file() {
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let filename = e.file_name().into_string().unwrap_or_default();
            let uidl = filename.split(':').next().unwrap_or(&filename).to_string();
            entries.push(MailEntry {
                path,
                size,
                filename,
                uidl,
            });
        }
    }
}
