use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use rand::rngs::OsRng;

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
            None => {
                let salt = SaltString::generate(&mut OsRng);
                let hasher = Argon2::default();
                let hash = hasher
                    .hash_password(password.as_bytes(), &salt)
                    .unwrap()
                    .to_string();
                match self.store.insert(username, hash.into_bytes()) {
                    Ok(_) => {
                        println!("creating new user {}", username);
                        Ok(true)
                    }
                    Err(e) => {
                        println!("error creating new user {} {}", username, e);
                        Err(e)
                    }
                }
            }
        }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<bool, sled::Error> {
        match self.store.get(username)? {
            Some(val) => {
                let stored_hash_str = std::str::from_utf8(&val).unwrap();
                let stored_hash = PasswordHash::new(stored_hash_str).unwrap();
                let is_valid = Argon2::default()
                    .verify_password(password.as_bytes(), &stored_hash)
                    .is_ok();
                Ok(is_valid)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_and_login() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let auth_store = AuthStore::new(db);

        let username = "testuser";
        let password = "testpassword123";

        let user_created = auth_store.create_user(username, password).unwrap();
        assert!(user_created, "User should be created successfully");

        let duplicate_user_created = auth_store.create_user(username, password).unwrap();
        assert!(
            !duplicate_user_created,
            "Creating duplicate user should return false"
        );

        let login_success = auth_store.login(username, password).unwrap();
        assert!(login_success, "Login with correct password should succeed");

        let login_fail = auth_store.login(username, "wrongpassword").unwrap();
        assert!(!login_fail, "Login with wrong password should fail");

        let login_nonexistent = auth_store.login("nonexistent", password).unwrap();
        assert!(
            !login_nonexistent,
            "Login with non-existent user should fail"
        );
    }
}
