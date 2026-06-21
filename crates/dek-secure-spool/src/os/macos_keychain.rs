use crate::key_manager::{KeyStoreError, OsKeyStore};

pub struct MacOsKeychainStore;

impl MacOsKeychainStore {
    pub fn new() -> Self {
        Self {}
    }
}

impl OsKeyStore for MacOsKeychainStore {
    fn load_or_create_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        // Fallback or skeleton for macOS.
        // In a real implementation, this would use security-framework crate to access the keychain.
        Err(KeyStoreError::Os(
            "macOS keychain not fully implemented in this demo".into(),
        ))
    }

    fn rotate_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        Err(KeyStoreError::Os(
            "macOS keychain not fully implemented in this demo".into(),
        ))
    }
}
