use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn init_user_mailbox(username: &str) -> std::io::Result<()> {
    let mailbox = format!("Maildir/{username}");
    fs::create_dir_all(format!("{mailbox}/cur"))?;
    fs::create_dir_all(format!("{mailbox}/new"))?;
    fs::create_dir_all(format!("{mailbox}/tmp"))?;
    Ok(())
}

pub struct MailDir {}

impl MailDir {
    pub fn list_messages(username: &str) -> Vec<Vec<MailEntry>> {
        let mut mail_entries = Vec::new();
        let path_new_str = format!("Maildir/{}/new", username);
        let path_new = Path::new(&path_new_str);
        let path_cur_str = format!("Maildir/{}/cur", username);
        let path_cur = Path::new(&path_cur_str);
        mail_entries.extend(scan_dir(path_new));
        mail_entries.extend(scan_dir(path_cur));
        return mail_entries;
    }
}

pub struct MailEntry {
    pub path: PathBuf,
    pub size: u64,
    pub filename: String,
}

pub fn scan_dir(dir: &Path) -> std::io::Result<Vec<MailEntry>> {
    let mut mail_entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let e = entry?;
        let path = e.path();
        if path.is_file() {
            let metadata = fs::metadata(&path)?;
            let size = metadata.len();
            let filename = e.file_name().into_string().unwrap_or_default();
            mail_entries.push(MailEntry {
                path,
                size,
                filename,
            });
        }
    }

    Ok(mail_entries)
}
