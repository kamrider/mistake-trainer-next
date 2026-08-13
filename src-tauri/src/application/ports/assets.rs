use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetDecryptionError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetEncryptionError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetBlobRemovalError;

/// Inbound features depend on this capability, never on the concrete asset cipher.
pub trait AssetDecryptor: Send + Sync {
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, AssetDecryptionError>;
}

/// Outbound encryption capability implemented at the infrastructure boundary.
pub trait AssetEncryptor: Send + Sync {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AssetEncryptionError>;
}

/// Read/write asset cryptography required by derivation transactions.
pub trait AssetCipher: AssetDecryptor + AssetEncryptor {}

impl<T> AssetCipher for T where T: AssetDecryptor + AssetEncryptor + ?Sized {}

/// Safe deletion capability for encrypted blobs after their owning transaction commits.
pub trait AssetBlobRemover: Send + Sync {
    fn remove(&self, blob_root: &Path, relative_path: &str) -> Result<bool, AssetBlobRemovalError>;
}
