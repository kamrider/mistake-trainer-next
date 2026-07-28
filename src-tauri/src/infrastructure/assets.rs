use std::{
    fmt::Write,
    fs, io,
    path::{Component, Path},
};

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

/// Removes an encrypted blob only when every existing path component is a
/// normal, non-reparse entry contained by `blob_root`.
pub fn remove_asset_blob(blob_root: &Path, relative_path: &str) -> io::Result<bool> {
    let relative = Path::new(relative_path);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset path is not a safe relative path",
        ));
    }

    let canonical_root = blob_root.canonicalize()?;
    let mut current = blob_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("relative path components were validated above");
        };
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "asset path crosses a link or reparse point",
            ));
        }
    }

    let canonical_path = current.canonicalize()?;
    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset path escapes the blob root",
        ));
    }
    fs::remove_file(canonical_path)?;
    Ok(true)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::remove_asset_blob;

    #[test]
    fn asset_removal_stays_inside_the_blob_root() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("blobs");
        std::fs::create_dir_all(root.join("aa")).expect("blob directory");
        std::fs::write(root.join("aa/blob.bin"), b"encrypted").expect("blob");
        std::fs::write(directory.path().join("outside.bin"), b"keep").expect("outside");

        assert!(remove_asset_blob(&root, "aa/blob.bin").expect("safe removal"));
        assert!(!root.join("aa/blob.bin").exists());
        assert!(remove_asset_blob(&root, "../outside.bin").is_err());
        assert!(directory.path().join("outside.bin").exists());
    }

    #[cfg(windows)]
    #[test]
    fn asset_removal_rejects_a_directory_junction_outside_the_blob_root() {
        use std::process::{Command, Stdio};

        let directory = tempfile::tempdir().expect("temp directory");
        let outside = tempfile::tempdir().expect("outside directory");
        let root = directory.path().join("blobs");
        std::fs::create_dir_all(&root).expect("blob root");
        std::fs::write(outside.path().join("keep.bin"), b"keep").expect("outside blob");
        let junction = root.join("escaped");
        let status = Command::new("cmd")
            .arg("/c")
            .arg("mklink")
            .arg("/J")
            .arg(&junction)
            .arg(outside.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("create junction fixture");
        assert!(status.success(), "junction fixture must be created");

        assert!(remove_asset_blob(&root, "escaped/keep.bin").is_err());
        assert!(outside.path().join("keep.bin").exists());
    }
}
