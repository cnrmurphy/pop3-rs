pub struct AuthStore {
    store: sled::Db,
}

impl AuthStore {
    pub fn new(store: sled::Db) -> Self {
        Self { store }
    }

    pub fn create_user(&self, username: &str, password: &str) -> Result<bool, sled::Error> {
        println!("attempting to create new user {}", username);
        match self.store.get(username)? {
            Some(val) => {
                println!(
                    "user {} exists already! {}",
                    username,
                    std::str::from_utf8(&val).unwrap()
                );
                Ok(false)
            }
            None => match self.store.insert(username, password) {
                Ok(_) => {
                    println!("creating new user {}", username);
                    Ok(true)
                }
                Err(e) => {
                    println!("error creating new user {} {}", username, e);
                    Err(e)
                }
            },
        }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<bool, sled::Error> {
        match self.store.get(username)? {
            Some(val) => {
                let stored_password = std::str::from_utf8(&val).unwrap();
                Ok(stored_password == password)
            }
            None => Ok(false),
        }
    }
}
