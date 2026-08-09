use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{BackupError, ManifestFile};

pub(super) fn reject_sqlite_sidecars(root: &Path) -> Result<(), BackupError> {
    for name in ["library.db-wal", "library.db-shm", "library.db-journal"] {
        match fs::symlink_metadata(root.join(name)) {
            Ok(_) => return Err(BackupError::InvalidPackage),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(BackupError::Integrity),
        }
    }
    Ok(())
}

pub(super) fn manifest_file_for_existing(
    path: &Path,
    relative_path: String,
    max_bytes: u64,
) -> Result<ManifestFile, BackupError> {
    let (encrypted_bytes, ciphertext_sha256) = hash_file(path, max_bytes)?;
    Ok(ManifestFile {
        relative_path,
        encrypted_bytes,
        ciphertext_sha256,
    })
}

pub(super) fn read_verified_manifest_file(
    root: &Path,
    entry: &ManifestFile,
    max_bytes: u64,
) -> Result<(PathBuf, Vec<u8>), BackupError> {
    let relative = safe_relative_path(&entry.relative_path)?;
    ensure_no_reparse_components(root, &relative)?;
    let canonical = canonical_contained_file(root, &relative)?;
    let bytes = read_bounded(&canonical, max_bytes)?;
    if u64::try_from(bytes.len()).ok() != Some(entry.encrypted_bytes)
        || sha256_bytes(&bytes) != entry.ciphertext_sha256
    {
        return Err(BackupError::Integrity);
    }
    Ok((canonical, bytes))
}

pub(super) fn canonical_contained_file(
    root: &Path,
    relative: &Path,
) -> Result<PathBuf, BackupError> {
    let canonical = root
        .join(relative)
        .canonicalize()
        .map_err(|_| BackupError::Integrity)?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(BackupError::Integrity);
    }
    Ok(canonical)
}

pub(super) fn ensure_no_reparse_components(
    root: &Path,
    relative: &Path,
) -> Result<(), BackupError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BackupError::InvalidPackage);
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| BackupError::Integrity)?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(BackupError::Integrity);
        }
    }
    Ok(())
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

pub(super) fn copy_and_hash(
    source: &Path,
    destination: &Path,
    max_bytes: u64,
) -> Result<(u64, String), BackupError> {
    let mut input = fs::File::open(source)?;
    if input.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|value| *value <= max_bytes)
            .ok_or(BackupError::TooLarge)?;
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
    }
    output.sync_all()?;
    Ok((total, format!("{:x}", digest.finalize())))
}

pub(super) fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn hash_file(path: &Path, max_bytes: u64) -> Result<(u64, String), BackupError> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|value| *value <= max_bytes)
            .ok_or(BackupError::TooLarge)?;
        digest.update(&buffer[..read]);
    }
    Ok((total, format!("{:x}", digest.finalize())))
}

pub(super) fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BackupError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(BackupError::TooLarge);
    }
    Ok(bytes)
}

pub(super) fn safe_relative_path(value: &str) -> Result<PathBuf, BackupError> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || !path.components().all(|component| match component {
            Component::Normal(value) => safe_windows_component(value),
            _ => false,
        })
    {
        return Err(BackupError::InvalidPackage);
    }
    Ok(path.to_path_buf())
}

fn safe_windows_component(value: &std::ffi::OsStr) -> bool {
    let Some(value) = value.to_str() else {
        return false;
    };
    if value.is_empty()
        || value.contains(':')
        || value.ends_with('.')
        || value.ends_with(' ')
        || value.chars().any(char::is_control)
    {
        return false;
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !matches!(
            stem.as_str(),
            "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        )
        && !matches!(
            stem.as_str(),
            "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
        )
}

pub(super) fn normalize_relative(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn safe_label(value: &str) -> String {
    let label = value
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>();
    if label.trim().is_empty() {
        "backup".to_owned()
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{copy_and_hash, safe_relative_path, sha256_bytes};
    use crate::modules::backup::BackupError;

    #[test]
    fn rejects_traversal_and_windows_reserved_components() {
        for value in [
            "../library.db",
            "assets/../../library.db",
            "CON",
            "assets/COM1.enc",
        ] {
            assert!(
                matches!(safe_relative_path(value), Err(BackupError::InvalidPackage)),
                "{value} must be rejected"
            );
        }
    }

    #[test]
    fn copying_is_bounded_and_hashes_the_written_bytes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.enc");
        let destination = directory.path().join("destination.enc");
        fs::write(&source, b"abc").expect("write source");

        let (bytes, digest) = copy_and_hash(&source, &destination, 3).expect("copy within budget");
        assert_eq!(bytes, 3);
        assert_eq!(digest, sha256_bytes(b"abc"));
        assert_eq!(fs::read(destination).expect("read copy"), b"abc");

        let oversized = directory.path().join("oversized.enc");
        assert!(matches!(
            copy_and_hash(&source, &oversized, 2),
            Err(BackupError::TooLarge)
        ));
    }
}
