use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{LegacyScanError, MAX_DIRECTORY_ENTRIES, MAX_RECORDS};

pub(super) const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
pub(in crate::modules::legacy) const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_TOTAL_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_DIRECTORY_DEPTH: usize = 32;
const MAX_FINGERPRINT_ENTRIES: usize = MAX_RECORDS + MAX_DIRECTORY_ENTRIES;

pub fn legacy_tree_fingerprint(root: &Path) -> Result<String, LegacyScanError> {
    if !root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }

    let mut visited_entries = 0;
    let mut files = Vec::new();
    collect_fingerprint_files(
        &canonical_root,
        &canonical_root,
        0,
        &mut visited_entries,
        &mut files,
    )?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut digest = Sha256::new();
    let mut total_bytes = 0_u64;
    for (relative, path) in files {
        let metadata = path.metadata()?;
        total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "legacy tree is too large")
        })?;
        if total_bytes > MAX_TOTAL_ASSET_BYTES.saturating_add(MAX_METADATA_BYTES) {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy tree is too large",
            )));
        }
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_le_bytes());
        let mut file = fs::File::open(path)?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        digest.update([0xff]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_fingerprint_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    visited_entries: &mut usize,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), LegacyScanError> {
    if depth > MAX_DIRECTORY_DEPTH {
        return Err(LegacyScanError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy tree is too deeply nested",
        )));
    }

    let remaining = MAX_FINGERPRINT_ENTRIES.saturating_sub(*visited_entries);
    let mut entries = fs::read_dir(directory)?
        .take(remaining.saturating_add(1))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.len() > remaining {
        return Err(LegacyScanError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy tree contains too many entries",
        )));
    }
    *visited_entries = visited_entries.saturating_add(entries.len());
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || is_windows_reparse_point(&path)? {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy tree contains a link or reparse point",
            )));
        }
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(root) {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy path resolves outside the selected directory",
            )));
        }
        if canonical.is_dir() {
            collect_fingerprint_files(
                root,
                &canonical,
                depth.saturating_add(1),
                visited_entries,
                files,
            )?;
        } else if canonical.is_file() {
            let relative = canonical
                .strip_prefix(root)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid legacy path"))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, canonical));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse_point(path: &Path) -> Result<bool, io::Error> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    Ok(fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_path: &Path) -> Result<bool, io::Error> {
    Ok(false)
}

pub(in crate::modules::legacy) fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[derive(Debug)]
pub(in crate::modules::legacy) enum BoundedReadError {
    Io,
    TooLarge,
}

pub(in crate::modules::legacy) fn read_bounded(
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, BoundedReadError> {
    let file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut contents = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)
        .map_err(|_| BoundedReadError::Io)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    Ok(contents)
}

pub(super) fn sha256_file(path: &Path, max_bytes: u64) -> Result<(String, u64), BoundedReadError> {
    let mut file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| BoundedReadError::Io)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(BoundedReadError::TooLarge)?;
        if total > max_bytes {
            return Err(BoundedReadError::TooLarge);
        }
        digest.update(&buffer[..read]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}
