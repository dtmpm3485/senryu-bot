
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};

#[derive(Clone)]
pub struct Crypto {
    cipher: Option<Aes256Gcm>,
}

impl Crypto {
    pub fn new(hex_key: &str) -> Result<Self> {
        let key = hex_key.trim();
        if key.is_empty() {
            return Ok(Self { cipher: None });
        }
        let bytes = hex::decode(key).context("encryption key must be hex")?;
        if bytes.len() != 32 {
            bail!("encryption key must be 64 hex characters (32 bytes)");
        }
        Ok(Self {
            cipher: Some(Aes256Gcm::new_from_slice(&bytes).expect("32-byte key")),
        })
    }

    pub fn enabled(&self) -> bool {
        self.cipher.is_some()
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let Some(cipher) = &self.cipher else {
            return Ok(plaintext.to_string());
        };

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let encrypted = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("AES-GCM encryption failed"))?;

        let mut out = Vec::with_capacity(nonce_bytes.len() + encrypted.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&encrypted);
        Ok(STANDARD.encode(out))
    }

    pub fn decrypt_if_needed(&self, input: &str) -> Result<String> {
        let Some(cipher) = &self.cipher else {
            return Ok(input.to_string());
        };
        let Ok(raw) = STANDARD.decode(input) else {
            return Ok(input.to_string());
        };
        if raw.len() < 12 + 16 {
            return Ok(input.to_string());
        }
        let (nonce_bytes, ciphertext) = raw.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        match cipher.decrypt(nonce, ciphertext) {
            Ok(v) => String::from_utf8(v).context("decrypted data is not UTF-8"),
            Err(_) => Ok(input.to_string()),
        }
    }

    pub fn is_encrypted(&self, input: &str) -> bool {
        let Some(cipher) = &self.cipher else {
            return false;
        };
        let Ok(raw) = STANDARD.decode(input) else {
            return false;
        };
        if raw.len() < 28 {
            return false;
        }
        let (nonce_bytes, ciphertext) = raw.split_at(12);
        cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext).is_ok()
    }
}
