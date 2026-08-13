use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    application::startup::initialize_application_library,
    infrastructure::{
        assets::AssetCryptoError,
        database::DatabaseError,
        runtime::{LibraryRuntime, RuntimeError, SecretStore, load_restore_credentials},
        storage_location::{
            STORAGE_PENDING_FILE, STORAGE_RECEIPT_FILE, StorageLocationError, read_control_json,
            remove_control_file, resolve_storage, write_control_json, write_storage_pointer,
        },
    },
};

#[path = "storage_migration_snapshot.rs"]
mod snapshot;

const JOURNAL_SCHEMA_VERSION: u32 = 1;
const OWNER_SCHEMA_VERSION: u32 = 1;
const PRODUCT_DIRECTORY: &str = "Mistake Trainer Next Data";
const LIBRARY_DIRECTORY: &str = "library";
const DATABASE_FILE: &str = "library.db";
const ASSETS_DIRECTORY: &str = "assets";
const OWNER_FILE: &str = ".mistake-trainer-storage.json";
const MAX_JOURNAL_AGE_MS: i64 = 24 * 60 * 60 * 1_000;
const MAX_OWNER_BYTES: u64 = 64 * 1024;
const MAX_DATABASE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 65 * 1024 * 1024;
const MAX_TOTAL_ASSET_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ASSETS: usize = 50_000;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageMigrationJournal {
    pub schema_version: u32,
    pub migration_id: String,
    pub source_library_root: PathBuf,
    pub destination_library_root: PathBuf,
    pub created_at_utc_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StorageMigrationOutcome {
    Scheduled,
    Moved,
    RolledBack,
    CleanupRequired,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageMigrationReceipt {
    pub outcome: StorageMigrationOutcome,
    pub destination_label: String,
    pub copied_asset_count: u32,
    pub copied_bytes: f64,
}

#[derive(Debug, Error)]
pub enum StorageMigrationError {
    #[error("the selected storage destination is invalid")]
    InvalidDestination,
    #[error("the selected storage destination is not empty")]
    DestinationInUse,
    #[error("another storage migration is already pending")]
    MigrationPending,
    #[error("the storage migration journal is invalid")]
    InvalidJournal,
    #[error("the storage migration journal has expired")]
    ExpiredJournal,
    #[error("the storage migration exceeds its safety budget")]
    TooLarge,
    #[error("the storage migration integrity check failed")]
    Integrity,
    #[error("the local library is busy")]
    Lock,
    #[error("a storage migration file operation failed")]
    File(#[from] io::Error),
    #[error("a storage migration database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("the encrypted migration database could not be opened")]
    DatabaseOpen(#[from] DatabaseError),
    #[error("the local library could not be reopened")]
    Runtime(#[from] RuntimeError),
    #[error("a storage control file operation failed")]
    Storage(#[from] StorageLocationError),
    #[error("an encrypted asset failed authentication")]
    Asset(#[from] AssetCryptoError),
}

impl StorageMigrationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidDestination => "storage_destination_invalid",
            Self::DestinationInUse => "storage_destination_in_use",
            Self::MigrationPending => "storage_migration_pending",
            Self::InvalidJournal => "storage_migration_journal_invalid",
            Self::ExpiredJournal => "storage_migration_journal_expired",
            Self::TooLarge => "storage_migration_too_large",
            Self::Integrity | Self::Asset(_) => "storage_migration_integrity_failed",
            Self::Lock => "storage_library_busy",
            Self::File(_) => "storage_copy_failed",
            Self::Database(_) | Self::DatabaseOpen(_) => "storage_database_failed",
            Self::Runtime(_) => "storage_runtime_failed",
            Self::Storage(error) => error.code(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StorageOwner {
    schema_version: u32,
    migration_id: String,
    account_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JournalPointerState {
    Source,
    Destination,
}

#[derive(Clone)]
pub struct StorageMigrationSource {
    connection: Arc<Mutex<Connection>>,
    blob_root: PathBuf,
    asset_key: [u8; 32],
    database_key: String,
    account_id: String,
}

impl std::fmt::Debug for StorageMigrationSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StorageMigrationSource")
            .field("connection", &"<encrypted database>")
            .field("blob_root", &"<application storage>")
            .field("asset_key", &"<redacted>")
            .field("database_key", &"<redacted>")
            .field("account_id", &"<redacted>")
            .finish()
    }
}

impl StorageMigrationSource {
    pub fn from_runtime(runtime: &LibraryRuntime) -> Self {
        Self {
            connection: Arc::clone(&runtime.connection),
            blob_root: runtime.blob_root.clone(),
            asset_key: runtime.asset_key,
            database_key: runtime.database_key().to_owned(),
            account_id: runtime.account_id().to_owned(),
        }
    }

    fn account_id(&self) -> &str {
        &self.account_id
    }

    fn database_key(&self) -> &str {
        &self.database_key
    }

    pub(crate) fn library_root(&self) -> Option<&Path> {
        self.blob_root.parent()
    }
}

pub fn stage_storage_migration(
    runtime: &LibraryRuntime,
    control_root: &Path,
    selected_parent: &Path,
    now_utc_ms: i64,
) -> Result<StorageMigrationReceipt, StorageMigrationError> {
    stage_storage_migration_from_source(
        &StorageMigrationSource::from_runtime(runtime),
        control_root,
        selected_parent,
        now_utc_ms,
    )
}

pub fn stage_storage_migration_from_source(
    source: &StorageMigrationSource,
    control_root: &Path,
    selected_parent: &Path,
    now_utc_ms: i64,
) -> Result<StorageMigrationReceipt, StorageMigrationError> {
    if read_pending_journal(control_root)?.is_some() {
        return Err(StorageMigrationError::MigrationPending);
    }

    let source_root = source
        .blob_root
        .parent()
        .ok_or(StorageMigrationError::InvalidDestination)?
        .canonicalize()
        .map_err(|_| StorageMigrationError::InvalidDestination)?;
    ensure_safe_directory(&source_root)?;
    ensure_safe_regular_file(&source_root.join(DATABASE_FILE))?;

    let configured = resolve_storage(control_root)?;
    let configured_source = canonical_existing(configured.library_root())?;
    if configured_source != source_root {
        return Err(StorageMigrationError::InvalidDestination);
    }

    let control_root = control_root
        .canonicalize()
        .map_err(|_| StorageMigrationError::InvalidDestination)?;
    let selected_parent = selected_parent
        .canonicalize()
        .map_err(|_| StorageMigrationError::InvalidDestination)?;
    ensure_no_link_or_reparse_ancestor(&selected_parent)?;
    ensure_safe_directory(&selected_parent)?;
    if fs::read_dir(&selected_parent)?
        .next()
        .transpose()?
        .is_some()
    {
        return Err(StorageMigrationError::DestinationInUse);
    }

    let destination_product_root = selected_parent.join(PRODUCT_DIRECTORY);
    let destination_library_root = destination_product_root.join(LIBRARY_DIRECTORY);
    ensure_disjoint_roots(
        &source_root,
        &control_root,
        &selected_parent,
        &destination_library_root,
    )?;

    let migration_id = Uuid::now_v7().to_string();
    let stage_root = selected_parent.join(format!(".mistake-trainer-migration-{migration_id}"));
    let stage_product_root = stage_root.join(PRODUCT_DIRECTORY);
    let stage_library_root = stage_product_root.join(LIBRARY_DIRECTORY);
    fs::create_dir(&stage_root)?;

    let mut final_created = false;
    let result = (|| {
        fs::create_dir(&stage_product_root)?;
        fs::create_dir(&stage_library_root)?;
        fs::create_dir(stage_library_root.join(ASSETS_DIRECTORY))?;
        write_owner(
            &stage_product_root,
            &StorageOwner {
                schema_version: OWNER_SCHEMA_VERSION,
                migration_id: migration_id.clone(),
                account_hash: account_hash(source.account_id()),
            },
        )?;

        // Keep the shared database connection exclusively locked from the snapshot through the
        // pending journal and fail-closed transition. A writer that was already waiting wakes only
        // after query_only is enabled, so it cannot commit data to the old root after the snapshot.
        let connection = source
            .connection
            .lock()
            .map_err(|_| StorageMigrationError::Lock)?;
        let (asset_count, copied_bytes) = snapshot::stage_library_snapshot(
            &connection,
            source,
            &stage_library_root,
            &stage_product_root,
            &migration_id,
        )?;

        fs::rename(&stage_product_root, &destination_product_root)?;
        final_created = true;
        fs::remove_dir(&stage_root)?;
        let receipt = StorageMigrationReceipt {
            outcome: StorageMigrationOutcome::Scheduled,
            destination_label: redacted_location_label(&selected_parent),
            copied_asset_count: u32::try_from(asset_count)
                .map_err(|_| StorageMigrationError::TooLarge)?,
            copied_bytes: copied_bytes as f64,
        };
        let journal = StorageMigrationJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            migration_id: migration_id.clone(),
            source_library_root: source_root.clone(),
            destination_library_root: destination_library_root.clone(),
            created_at_utc_ms: now_utc_ms,
        };
        write_control_json(
            control_root.as_path(),
            STORAGE_PENDING_FILE,
            &journal,
            false,
        )?;
        if let Err(error) = connection.pragma_update(None, "query_only", true) {
            let _ = remove_control_file(control_root.as_path(), STORAGE_PENDING_FILE);
            return Err(StorageMigrationError::Database(error));
        }

        Ok(receipt)
    })();

    if result.is_err() {
        if final_created {
            let _ = remove_owned_product_tree(
                &destination_product_root,
                &migration_id,
                source.account_id(),
            );
        }
        let _ = remove_owned_stage_tree(&stage_root);
    }
    result
}

pub fn storage_usage_bytes(
    source: &StorageMigrationSource,
) -> Result<(u64, u64), StorageMigrationError> {
    snapshot::storage_usage_bytes(source)
}

pub fn storage_migration_pending(control_root: &Path) -> Result<bool, StorageMigrationError> {
    read_pending_journal(control_root).map(|journal| journal.is_some())
}

pub fn apply_pending_storage_migration(
    control_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<Option<LibraryRuntime>, StorageMigrationError> {
    let Some(journal) = read_pending_journal(control_root)? else {
        return Ok(None);
    };
    let _pointer_state = validate_journal(control_root, &journal, now_utc_ms)?;
    let credentials = load_restore_credentials(secrets)?;
    let source_root = journal
        .source_library_root
        .canonicalize()
        .map_err(|_| StorageMigrationError::InvalidJournal)?;
    let destination_product_root = journal
        .destination_library_root
        .parent()
        .ok_or(StorageMigrationError::InvalidJournal)?;

    let destination_validation = snapshot::validate_library_tree(
        &journal.destination_library_root,
        &credentials.database_key,
        &credentials.asset_key,
        &credentials.account_id,
        Some(destination_product_root),
        Some(&journal.migration_id),
    );
    if destination_validation.is_err() {
        let cleanup_succeeded = remove_owned_product_tree(
            destination_product_root,
            &journal.migration_id,
            &credentials.account_id,
        )
        .is_ok();
        remove_control_file(control_root, STORAGE_PENDING_FILE)?;
        write_migration_receipt(
            control_root,
            StorageMigrationReceipt {
                outcome: if cleanup_succeeded {
                    StorageMigrationOutcome::RolledBack
                } else {
                    StorageMigrationOutcome::CleanupRequired
                },
                destination_label: redacted_location_label(
                    destination_product_root
                        .parent()
                        .ok_or(StorageMigrationError::InvalidJournal)?,
                ),
                copied_asset_count: 0,
                copied_bytes: 0.0,
            },
        )?;
        return initialize_application_library(&source_root, secrets, now_utc_ms)
            .map(Some)
            .map_err(|error| match error {
                crate::application::startup::StartupError::Runtime(error) => {
                    StorageMigrationError::Runtime(error)
                }
                _ => StorageMigrationError::Integrity,
            });
    }

    let runtime =
        initialize_application_library(&journal.destination_library_root, secrets, now_utc_ms)
            .map_err(|error| match error {
                crate::application::startup::StartupError::Runtime(error) => {
                    StorageMigrationError::Runtime(error)
                }
                _ => StorageMigrationError::Integrity,
            })?;
    let copied = library_size_summary(&journal.destination_library_root)?;
    write_storage_pointer(control_root, &journal.destination_library_root)?;
    remove_control_file(control_root, STORAGE_PENDING_FILE)?;

    let mut receipt = StorageMigrationReceipt {
        outcome: StorageMigrationOutcome::Moved,
        destination_label: redacted_location_label(
            destination_product_root
                .parent()
                .ok_or(StorageMigrationError::InvalidJournal)?,
        ),
        copied_asset_count: copied.0,
        copied_bytes: copied.1 as f64,
    };
    write_migration_receipt(control_root, receipt.clone())?;

    if remove_committed_source(control_root, &source_root).is_err() {
        receipt.outcome = StorageMigrationOutcome::CleanupRequired;
        write_migration_receipt(control_root, receipt)?;
    }
    Ok(Some(runtime))
}

pub fn read_storage_migration_receipt(
    control_root: &Path,
) -> Result<Option<StorageMigrationReceipt>, StorageMigrationError> {
    let receipt = read_control_json::<StorageMigrationReceipt>(control_root, STORAGE_RECEIPT_FILE)
        .map_err(|error| match error {
            StorageLocationError::InvalidPointer => StorageMigrationError::InvalidJournal,
            other => StorageMigrationError::Storage(other),
        })?;
    if let Some(receipt) = &receipt
        && (receipt.destination_label.trim().is_empty()
            || !receipt.copied_bytes.is_finite()
            || receipt.copied_bytes < 0.0)
    {
        return Err(StorageMigrationError::InvalidJournal);
    }
    Ok(receipt)
}

pub fn take_storage_migration_receipt(
    control_root: &Path,
) -> Result<Option<StorageMigrationReceipt>, StorageMigrationError> {
    let receipt = read_storage_migration_receipt(control_root)?;
    if receipt.is_some() {
        remove_control_file(control_root, STORAGE_RECEIPT_FILE)?;
    }
    Ok(receipt)
}

fn read_pending_journal(
    control_root: &Path,
) -> Result<Option<StorageMigrationJournal>, StorageMigrationError> {
    read_control_json(control_root, STORAGE_PENDING_FILE).map_err(|error| match error {
        StorageLocationError::InvalidPointer => StorageMigrationError::InvalidJournal,
        other => StorageMigrationError::Storage(other),
    })
}

fn validate_journal(
    control_root: &Path,
    journal: &StorageMigrationJournal,
    now_utc_ms: i64,
) -> Result<JournalPointerState, StorageMigrationError> {
    if journal.schema_version != JOURNAL_SCHEMA_VERSION
        || Uuid::parse_str(&journal.migration_id)
            .ok()
            .filter(|value| value.to_string() == journal.migration_id)
            .is_none()
        || !journal.source_library_root.is_absolute()
        || !journal.destination_library_root.is_absolute()
        || !has_product_owned_suffix(&journal.destination_library_root)
    {
        return Err(StorageMigrationError::InvalidJournal);
    }
    let age = now_utc_ms
        .checked_sub(journal.created_at_utc_ms)
        .ok_or(StorageMigrationError::ExpiredJournal)?;
    if !(0..=MAX_JOURNAL_AGE_MS).contains(&age) {
        return Err(StorageMigrationError::ExpiredJournal);
    }

    let configured = resolve_storage(control_root)?;
    let configured_root = canonical_existing(configured.library_root())?;
    let journal_source = canonical_existing(&journal.source_library_root)?;
    let journal_destination = canonical_existing(&journal.destination_library_root)?;
    let pointer_state = if configured_root == journal_source {
        JournalPointerState::Source
    } else if configured_root == journal_destination {
        JournalPointerState::Destination
    } else {
        return Err(StorageMigrationError::InvalidJournal);
    };
    let control = canonical_existing(control_root)?;
    let destination_parent = journal
        .destination_library_root
        .parent()
        .and_then(Path::parent)
        .ok_or(StorageMigrationError::InvalidJournal)?;
    let destination_parent = canonical_existing(destination_parent)?;
    ensure_disjoint_roots(
        &journal_source,
        &control,
        &destination_parent,
        &journal.destination_library_root,
    )?;
    Ok(pointer_state)
}

fn write_migration_receipt(
    control_root: &Path,
    receipt: StorageMigrationReceipt,
) -> Result<(), StorageMigrationError> {
    write_control_json(control_root, STORAGE_RECEIPT_FILE, &receipt, true)?;
    Ok(())
}

fn write_owner(product_root: &Path, owner: &StorageOwner) -> Result<(), StorageMigrationError> {
    let bytes = serde_json::to_vec_pretty(owner).map_err(|_| StorageMigrationError::Integrity)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_OWNER_BYTES {
        return Err(StorageMigrationError::TooLarge);
    }
    let path = product_root.join(OWNER_FILE);
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn validate_owner(
    product_root: &Path,
    migration_id: &str,
    account_id: &str,
) -> Result<(), StorageMigrationError> {
    ensure_safe_directory(product_root)?;
    let owner: StorageOwner = read_bounded_json(&product_root.join(OWNER_FILE), MAX_OWNER_BYTES)?;
    if owner.schema_version != OWNER_SCHEMA_VERSION
        || owner.migration_id != migration_id
        || owner.account_hash != account_hash(account_id)
    {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(())
}

fn read_bounded_json<T: DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> Result<T, StorageMigrationError> {
    ensure_safe_regular_file(path)?;
    let metadata = fs::metadata(path)?;
    if metadata.len() > max_bytes {
        return Err(StorageMigrationError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    File::open(path)?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(StorageMigrationError::TooLarge);
    }
    serde_json::from_slice(&bytes).map_err(|_| StorageMigrationError::Integrity)
}

fn remove_owned_product_tree(
    product_root: &Path,
    migration_id: &str,
    account_id: &str,
) -> Result<(), StorageMigrationError> {
    validate_owner(product_root, migration_id, account_id)?;
    ensure_tree_has_no_links(product_root)?;
    fs::remove_dir_all(product_root)?;
    Ok(())
}

fn remove_owned_stage_tree(stage_root: &Path) -> Result<(), StorageMigrationError> {
    let metadata = match fs::symlink_metadata(stage_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(StorageMigrationError::Integrity);
    }
    ensure_tree_has_no_links(stage_root)?;
    fs::remove_dir_all(stage_root)?;
    Ok(())
}

fn remove_committed_source(
    control_root: &Path,
    source_root: &Path,
) -> Result<(), StorageMigrationError> {
    let canonical_source = canonical_existing(source_root)?;
    let control = canonical_existing(control_root)?;
    let default_source = control.join(LIBRARY_DIRECTORY);
    let is_default = default_source.exists()
        && default_source
            .canonicalize()
            .map(|path| path == canonical_source)
            .unwrap_or(false);
    if !is_default && !has_product_owned_suffix(&canonical_source) {
        return Err(StorageMigrationError::Integrity);
    }
    if canonical_source == control || control.starts_with(&canonical_source) {
        return Err(StorageMigrationError::Integrity);
    }
    ensure_safe_regular_file(&canonical_source.join(DATABASE_FILE))?;
    ensure_tree_has_no_links(&canonical_source)?;
    fs::remove_dir_all(&canonical_source)?;
    Ok(())
}

fn ensure_tree_has_no_links(root: &Path) -> Result<(), StorageMigrationError> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
        return Err(StorageMigrationError::Integrity);
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(root)? {
            ensure_tree_has_no_links(&entry?.path())?;
        }
    }
    Ok(())
}

fn library_size_summary(library_root: &Path) -> Result<(u32, u64), StorageMigrationError> {
    let database_bytes = fs::metadata(library_root.join(DATABASE_FILE))?.len();
    let files = collect_file_metadata(&library_root.join(ASSETS_DIRECTORY))?;
    let asset_count = u32::try_from(files.len()).map_err(|_| StorageMigrationError::TooLarge)?;
    let total = files
        .into_iter()
        .try_fold(database_bytes, |total, (_, bytes)| {
            total
                .checked_add(bytes)
                .ok_or(StorageMigrationError::TooLarge)
        })?;
    Ok((asset_count, total))
}

fn collect_relative_files(root: &Path) -> Result<HashSet<String>, StorageMigrationError> {
    Ok(collect_file_metadata(root)?
        .into_iter()
        .map(|(path, _)| path)
        .collect())
}

fn collect_file_metadata(root: &Path) -> Result<Vec<(String, u64)>, StorageMigrationError> {
    ensure_safe_directory(root)?;
    let mut files = Vec::new();
    collect_file_metadata_inner(root, root, &mut files)?;
    Ok(files)
}

fn collect_file_metadata_inner(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, u64)>,
) -> Result<(), StorageMigrationError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(StorageMigrationError::Integrity);
        }
        if metadata.is_dir() {
            collect_file_metadata_inner(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StorageMigrationError::Integrity)?;
            files.push((normalize_relative(relative), metadata.len()));
            if files.len() > MAX_ASSETS {
                return Err(StorageMigrationError::TooLarge);
            }
        } else {
            return Err(StorageMigrationError::Integrity);
        }
    }
    Ok(())
}

fn ensure_disjoint_roots(
    source_root: &Path,
    control_root: &Path,
    selected_parent: &Path,
    destination_library_root: &Path,
) -> Result<(), StorageMigrationError> {
    if roots_overlap(source_root, selected_parent)
        || roots_overlap(control_root, selected_parent)
        || roots_overlap(source_root, destination_library_root)
        || roots_overlap(control_root, destination_library_root)
    {
        return Err(StorageMigrationError::InvalidDestination);
    }
    Ok(())
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn canonical_existing(path: &Path) -> Result<PathBuf, StorageMigrationError> {
    path.canonicalize()
        .map_err(|_| StorageMigrationError::InvalidDestination)
}

fn safe_relative_asset_path(value: &str) -> Result<PathBuf, StorageMigrationError> {
    if value.is_empty() || value.contains(':') || value.contains('\\') {
        return Err(StorageMigrationError::Integrity);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(path.to_path_buf())
}

fn ensure_no_reparse_components(root: &Path, relative: &Path) -> Result<(), StorageMigrationError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(StorageMigrationError::Integrity);
        };
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| StorageMigrationError::Integrity)?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(StorageMigrationError::Integrity);
        }
    }
    Ok(())
}

fn ensure_no_link_or_reparse_ancestor(path: &Path) -> Result<(), StorageMigrationError> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|_| StorageMigrationError::InvalidDestination)?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(StorageMigrationError::InvalidDestination);
        }
    }
    Ok(())
}

fn ensure_safe_directory(path: &Path) -> Result<(), StorageMigrationError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| StorageMigrationError::InvalidDestination)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(StorageMigrationError::InvalidDestination);
    }
    Ok(())
}

fn ensure_safe_regular_file(path: &Path) -> Result<(), StorageMigrationError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| StorageMigrationError::Integrity)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(())
}

fn hash_file(path: &Path, max_bytes: u64) -> Result<(u64, Vec<u8>), StorageMigrationError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).map_err(|_| StorageMigrationError::TooLarge)?)
            .filter(|value| *value <= max_bytes)
            .ok_or(StorageMigrationError::TooLarge)?;
        digest.update(&buffer[..count]);
    }
    Ok((total, digest.finalize().to_vec()))
}

fn account_hash(account_id: &str) -> String {
    format!("{:x}", Sha256::digest(account_id.as_bytes()))
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn has_product_owned_suffix(path: &Path) -> bool {
    path.file_name().and_then(|value| value.to_str()) == Some(LIBRARY_DIRECTORY)
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some(PRODUCT_DIRECTORY)
}

fn redacted_location_label(selected_parent: &Path) -> String {
    selected_parent
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("自定义位置 · {value}"))
        .unwrap_or_else(|| "自定义位置".to_owned())
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
