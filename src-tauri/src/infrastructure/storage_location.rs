use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const STORAGE_POINTER_FILE: &str = "storage-location.json";
pub const STORAGE_PENDING_FILE: &str = "storage-migration-pending.json";
pub const STORAGE_RECEIPT_FILE: &str = "storage-migration-receipt.json";

const STORAGE_POINTER_SCHEMA_VERSION: u32 = 1;
const MAX_CONTROL_FILE_BYTES: u64 = 64 * 1024;
const PRODUCT_DIRECTORY: &str = "Mistake Trainer Next Data";
const LIBRARY_DIRECTORY: &str = "library";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoragePointer {
    pub schema_version: u32,
    pub library_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedStorage {
    Default { library_root: PathBuf },
    Custom { library_root: PathBuf },
}

impl ResolvedStorage {
    pub fn library_root(&self) -> &Path {
        match self {
            Self::Default { library_root } | Self::Custom { library_root } => library_root,
        }
    }

    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom { .. })
    }
}

#[derive(Debug, Error)]
pub enum StorageLocationError {
    #[error("the storage pointer could not be read or persisted")]
    File(#[from] std::io::Error),
    #[error("the storage pointer is malformed or unsafe")]
    InvalidPointer,
    #[error("the configured storage location is unavailable")]
    Unavailable,
    #[error("a storage control operation is already pending")]
    ControlFileExists,
}

impl StorageLocationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::File(_) => "storage_pointer_io_failed",
            Self::InvalidPointer => "storage_pointer_invalid",
            Self::Unavailable => "storage_location_unavailable",
            Self::ControlFileExists => "storage_control_file_exists",
        }
    }
}

pub fn resolve_storage(control_root: &Path) -> Result<ResolvedStorage, StorageLocationError> {
    let pointer_path = control_root.join(STORAGE_POINTER_FILE);
    match fs::symlink_metadata(&pointer_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ResolvedStorage::Default {
                library_root: control_root.join(LIBRARY_DIRECTORY),
            });
        }
        Err(error) => return Err(StorageLocationError::File(error)),
    }

    let pointer = read_pointer_strict(&pointer_path)?;
    validate_custom_library_root(control_root, &pointer.library_root)?;
    Ok(ResolvedStorage::Custom {
        library_root: pointer.library_root,
    })
}

pub fn write_storage_pointer(
    control_root: &Path,
    library_root: &Path,
) -> Result<(), StorageLocationError> {
    validate_control_root(control_root)?;
    validate_custom_library_root(control_root, library_root)?;

    write_control_json(
        control_root,
        STORAGE_POINTER_FILE,
        &StoragePointer {
            schema_version: STORAGE_POINTER_SCHEMA_VERSION,
            library_root: library_root.to_path_buf(),
        },
        true,
    )
}

pub(crate) fn write_control_json<T: Serialize>(
    control_root: &Path,
    file_name: &str,
    value: &T,
    replace: bool,
) -> Result<(), StorageLocationError> {
    validate_control_file_name(file_name)?;
    validate_control_root(control_root)?;
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| StorageLocationError::InvalidPointer)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CONTROL_FILE_BYTES {
        return Err(StorageLocationError::InvalidPointer);
    }

    let target = control_root.join(file_name);
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || is_windows_reparse_point(&metadata)
            {
                return Err(StorageLocationError::InvalidPointer);
            }
            if !replace {
                return Err(StorageLocationError::ControlFileExists);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(StorageLocationError::File(error)),
    }
    let temporary = control_root.join(format!(".{file_name}.{}.tmp", Uuid::now_v7().simple()));
    write_new_synced(&temporary, &bytes)?;
    let replace_result = replace_file_atomically(&temporary, &target);
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    replace_result?;
    sync_parent_directory(control_root)?;
    Ok(())
}

pub(crate) fn read_control_json<T: DeserializeOwned>(
    control_root: &Path,
    file_name: &str,
) -> Result<Option<T>, StorageLocationError> {
    validate_control_file_name(file_name)?;
    validate_control_root(control_root)?;
    let path = control_root.join(file_name);
    match fs::symlink_metadata(&path) {
        Ok(_) => read_json_strict(&path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(StorageLocationError::File(error)),
    }
}

pub(crate) fn remove_control_file(
    control_root: &Path,
    file_name: &str,
) -> Result<(), StorageLocationError> {
    validate_control_file_name(file_name)?;
    validate_control_root(control_root)?;
    let path = control_root.join(file_name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(StorageLocationError::File(error)),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(StorageLocationError::InvalidPointer);
    }
    fs::remove_file(path)?;
    sync_parent_directory(control_root)?;
    Ok(())
}

fn read_pointer_strict(path: &Path) -> Result<StoragePointer, StorageLocationError> {
    let pointer: StoragePointer = read_json_strict(path)?;
    if pointer.schema_version != STORAGE_POINTER_SCHEMA_VERSION
        || !pointer.library_root.is_absolute()
        || !has_product_owned_suffix(&pointer.library_root)
    {
        return Err(StorageLocationError::InvalidPointer);
    }
    Ok(pointer)
}

fn read_json_strict<T: DeserializeOwned>(path: &Path) -> Result<T, StorageLocationError> {
    let metadata = fs::symlink_metadata(path).map_err(StorageLocationError::File)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
        || metadata.len() > MAX_CONTROL_FILE_BYTES
    {
        return Err(StorageLocationError::InvalidPointer);
    }

    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(usize::MAX)
            .min(MAX_CONTROL_FILE_BYTES as usize),
    );
    Read::by_ref(&mut file)
        .take(MAX_CONTROL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CONTROL_FILE_BYTES {
        return Err(StorageLocationError::InvalidPointer);
    }

    serde_json::from_slice(&bytes).map_err(|_| StorageLocationError::InvalidPointer)
}

fn validate_control_root(control_root: &Path) -> Result<(), StorageLocationError> {
    let metadata = fs::symlink_metadata(control_root).map_err(StorageLocationError::File)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(StorageLocationError::InvalidPointer);
    }
    Ok(())
}

fn validate_custom_library_root(
    control_root: &Path,
    library_root: &Path,
) -> Result<(), StorageLocationError> {
    if !library_root.is_absolute() || !has_product_owned_suffix(library_root) {
        return Err(StorageLocationError::InvalidPointer);
    }

    ensure_no_link_or_reparse_ancestor(library_root)?;
    let product_root = library_root
        .parent()
        .ok_or(StorageLocationError::InvalidPointer)?;
    for path in [product_root, library_root] {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageLocationError::Unavailable
            } else {
                StorageLocationError::File(error)
            }
        })?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || is_windows_reparse_point(&metadata)
        {
            return Err(StorageLocationError::InvalidPointer);
        }
    }

    let database_path = library_root.join("library.db");
    let database_metadata = fs::symlink_metadata(&database_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StorageLocationError::Unavailable
        } else {
            StorageLocationError::File(error)
        }
    })?;
    if !database_metadata.is_file()
        || database_metadata.file_type().is_symlink()
        || is_windows_reparse_point(&database_metadata)
    {
        return Err(StorageLocationError::InvalidPointer);
    }

    let canonical_control = control_root
        .canonicalize()
        .map_err(StorageLocationError::File)?;
    let canonical_library = library_root.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StorageLocationError::Unavailable
        } else {
            StorageLocationError::File(error)
        }
    })?;
    if canonical_library.starts_with(&canonical_control)
        || canonical_control.starts_with(&canonical_library)
    {
        return Err(StorageLocationError::InvalidPointer);
    }
    Ok(())
}

fn ensure_no_link_or_reparse_ancestor(path: &Path) -> Result<(), StorageLocationError> {
    for component_path in path.ancestors() {
        let metadata = fs::symlink_metadata(component_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageLocationError::Unavailable
            } else {
                StorageLocationError::File(error)
            }
        })?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(StorageLocationError::InvalidPointer);
        }
    }
    Ok(())
}

fn has_product_owned_suffix(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(LIBRARY_DIRECTORY)
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(PRODUCT_DIRECTORY)
}

fn validate_control_file_name(file_name: &str) -> Result<(), StorageLocationError> {
    if matches!(
        file_name,
        STORAGE_POINTER_FILE | STORAGE_PENDING_FILE | STORAGE_RECEIPT_FILE
    ) {
        Ok(())
    } else {
        Err(StorageLocationError::InvalidPointer)
    }
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), StorageLocationError> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn replace_file_atomically(source: &Path, target: &Path) -> Result<(), StorageLocationError> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR::from_raw(source.as_ptr()),
            PCWSTR::from_raw(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| StorageLocationError::File(std::io::Error::other(error.to_string())))
    }
}

#[cfg(not(windows))]
fn replace_file_atomically(source: &Path, target: &Path) -> Result<(), StorageLocationError> {
    fs::rename(source, target)?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), StorageLocationError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), StorageLocationError> {
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
