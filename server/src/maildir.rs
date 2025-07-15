use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub fn init_user_mailbox(username: &str) -> std::io::Result<()> {
    let mailbox = format!("Maildir/{username}");
    fs::create_dir_all(format!("{mailbox}/cur"))?;
    fs::create_dir_all(format!("{mailbox}/new"))?;
    fs::create_dir_all(format!("{mailbox}/tmp"))?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum MailDirError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("mail entry not found")]
    MailEntryNotFound(String),
}

pub struct MailDir {
    username: String,
    mailbox_new: PathBuf,
    mailbox_cur: PathBuf,
    // cache could easily be a vec since it maps well to how pop3 expects to retrieve messages. The
    // minor nuance that pop3 expects 1-based indexing and Vec is 0-based indexing makes me lean
    // toward using an ordered map to simplify things
    cache: BTreeMap<u64, MailEntry>,
}

impl MailDir {
    pub fn new(username: &str) -> std::io::Result<Self> {
        let path_new = PathBuf::from(format!("Maildir/{}/new", username));
        let path_cur = PathBuf::from(format!("Maildir/{}/cur", username));

        let mut maildir = Self {
            username: username.to_string(),
            mailbox_new: path_new,
            mailbox_cur: path_cur,
            cache: BTreeMap::new(),
        };
        maildir.refresh_cache();
        Ok(maildir)
    }

    pub fn refresh_cache(&mut self) {
        let mut cache = BTreeMap::new();
        let entries = self.list_messages();
        for entry in entries {
            Some(cache.insert(entry.id, entry));
        }
        self.cache = cache;
    }

    pub fn list_messages(&self) -> Vec<MailEntry> {
        let mut mail_entries = Vec::new();
        mail_entries.extend(scan_dir(&self.mailbox_new).into_iter());
        mail_entries.extend(scan_dir(&self.mailbox_cur).into_iter());
        return mail_entries.into_iter().flatten().collect();
    }

    pub fn read_message(&self, id: u64) -> Result<String, MailDirError> {
        match self.cache.get(&id) {
            Some(entry) => match fs::read_to_string(&entry.path) {
                Ok(mail_content) => Ok(mail_content),
                Err(e) => Err(MailDirError::IoError(e)),
            },
            None => Err(MailDirError::MailEntryNotFound(
                "mail entry not in cache".to_string(),
            )),
        }
    }
}

pub struct MailEntry {
    pub id: u64,
    pub path: PathBuf,
    pub size: u64,
    pub filename: String,
}

pub fn scan_dir(dir: &Path) -> std::io::Result<Vec<MailEntry>> {
    let mut mail_entries = Vec::new();
    let mut id = 1;
    for entry in fs::read_dir(dir)? {
        let e = entry?;
        let path = e.path();
        if path.is_file() {
            let metadata = fs::metadata(&path)?;
            let size = metadata.len();
            let filename = e.file_name().into_string().unwrap_or_default();
            mail_entries.push(MailEntry {
                id,
                path,
                size,
                filename,
            });
            id += 1;
        }
    }

    Ok(mail_entries)
}
