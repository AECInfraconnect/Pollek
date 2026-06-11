#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption failed")]
    Encrypt,
    #[error("decryption failed")]
    Decrypt,
    #[error("invalid key length")]
    InvalidKey,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecordAad {
    pub schema: String,
    pub tenant_id: String,
    pub device_id: String,
    pub segment_id: String,
    pub seq: u64,
    pub key_id: String,
    pub alg: String,
}

impl RecordAad {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self)
            .unwrap_or_else(|e| panic!("AAD serialization must not fail: {}", e))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedRecord {
    pub alg: String,
    pub key_id: String,
    pub nonce: [u8; 12],
    pub aad: RecordAad,
    pub ciphertext: Vec<u8>,
}

pub struct AeadKey {
    key_id: String,
    key_bytes: [u8; 32],
}

impl Drop for AeadKey {
    fn drop(&mut self) {
        self.key_bytes.zeroize();
    }
}

impl AeadKey {
    pub fn new(key_id: impl Into<String>, key_bytes: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            key_bytes,
        }
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn encrypt_record(
        &self,
        aad: RecordAad,
        plaintext: &[u8],
    ) -> Result<EncryptedRecord, CryptoError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key_bytes));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let aad_bytes = aad.to_bytes();

        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad_bytes,
                },
            )
            .map_err(|_| CryptoError::Encrypt)?;

        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(nonce.as_slice());

        Ok(EncryptedRecord {
            alg: "AES-256-GCM".to_string(),
            key_id: self.key_id.clone(),
            nonce: nonce_arr,
            aad,
            ciphertext,
        })
    }

    pub fn decrypt_record(&self, record: &EncryptedRecord) -> Result<Vec<u8>, CryptoError> {
        if record.alg != "AES-256-GCM" || record.key_id != self.key_id {
            return Err(CryptoError::Decrypt);
        }

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key_bytes));
        let nonce = Nonce::from_slice(&record.nonce);
        let aad_bytes = record.aad.to_bytes();

        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &record.ciphertext,
                    aad: &aad_bytes,
                },
            )
            .map_err(|_| CryptoError::Decrypt)
    }
}
