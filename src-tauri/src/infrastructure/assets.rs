use std::fmt::Write;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAGIC: &[u8; 4] = b"MTB1";
const NONCE_LENGTH: usize = 12;
const TAG_LENGTH: usize = 16;

#[derive(Debug, Error)]
pub enum AssetCryptoError {
    #[error("asset blob has an invalid format")]
    InvalidFormat,
    #[error("asset authentication failed")]
    Authentication,
    #[error("secure random generation failed")]
    Random,
}

pub fn plaintext_sha256(plaintext: &[u8]) -> String {
    let digest = Sha256::digest(plaintext);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

pub fn encrypt_asset(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, AssetCryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AssetCryptoError::Authentication)?;
    let mut nonce_bytes = [0_u8; NONCE_LENGTH];
    getrandom::fill(&mut nonce_bytes).map_err(|_| AssetCryptoError::Random)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| AssetCryptoError::Authentication)?;

    let mut blob = Vec::with_capacity(MAGIC.len() + NONCE_LENGTH + ciphertext.len());
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

pub fn decrypt_asset(blob: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, AssetCryptoError> {
    let header_length = MAGIC.len() + NONCE_LENGTH;
    if blob.len() < header_length + TAG_LENGTH || &blob[..MAGIC.len()] != MAGIC {
        return Err(AssetCryptoError::InvalidFormat);
    }

    let nonce = Nonce::from_slice(&blob[MAGIC.len()..header_length]);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AssetCryptoError::Authentication)?;
    cipher
        .decrypt(nonce, &blob[header_length..])
        .map_err(|_| AssetCryptoError::Authentication)
}
