use keyring::{Entry, Error as KeyringError};

use crate::error::DesktopError;

const SERVICE: &str = "com.itsvic.deepseek-harness-desktop";

#[derive(Default)]
pub struct SecureStore;

impl SecureStore {
    pub fn get(&self, key: &str) -> Result<Option<String>, DesktopError> {
        validate_key(key)?;
        match Entry::new(SERVICE, key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), DesktopError> {
        validate_key(key)?;
        Entry::new(SERVICE, key)?.set_password(value)?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<(), DesktopError> {
        validate_key(key)?;
        match Entry::new(SERVICE, key)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn validate_key(key: &str) -> Result<(), DesktopError> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(DesktopError::Other("invalid Keychain key".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_key;

    #[test]
    fn keychain_keys_are_narrowly_scoped() {
        assert!(validate_key("deepseek-api-key").is_ok());
        assert!(validate_key("../other-service").is_err());
    }
}
