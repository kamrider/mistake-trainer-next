use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::database::{DatabaseError, open_encrypted_database, run_migrations},
    modules::profiles::{
        CreateProfile, LearnerProfile, ProfileUseCaseError, create_profile, persist_active_profile,
    },
};

const DATABASE_KEY: &str = "database-key";
const ASSET_KEY: &str = "asset-key";
const ACCOUNT_ID: &str = "account-id";
const DEVICE_ID: &str = "device-id";

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
}

pub struct KeyringSecretStore {
    service: &'static str,
}

impl KeyringSecretStore {
    pub const fn new(service: &'static str) -> Self {
        Self { service }
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, name).map_err(|error| error.to_string())
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        match self.entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.entry(name)?
            .set_password(value)
            .map_err(|error| error.to_string())
    }
}

pub struct LibraryRuntime {
    pub connection: Arc<Mutex<Connection>>,
    pub blob_root: PathBuf,
    pub asset_key: [u8; 32],
    database_key: String,
    account_id: String,
    device_id: String,
    active_profile: RwLock<ActiveProfile>,
    profile_transition: Arc<Mutex<()>>,
}

pub(crate) struct RestoreCredentials {
    pub database_key: String,
    pub asset_key: [u8; 32],
    pub account_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveProfile {
    pub id: String,
    pub name: String,
}

impl std::fmt::Debug for LibraryRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryRuntime")
            .field("connection", &"<encrypted database>")
            .field("blob_root", &"<application storage>")
            .field("asset_key", &"<redacted>")
            .field("database_key", &"<redacted>")
            .field("account_id", &"<redacted>")
            .field("device_id", &"<redacted>")
            .field("active_profile", &"<redacted>")
            .field("profile_transition", &"<coordination lock>")
            .finish()
    }
}

impl LibraryRuntime {
    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn active_profile(&self) -> ActiveProfile {
        self.active_profile
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn lock_profile_transition(&self) -> MutexGuard<'_, ()> {
        self.profile_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn profile_transition_lock(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.profile_transition)
    }

    pub fn activate_profile(
        &self,
        profile_id: &str,
        now_utc_ms: i64,
    ) -> Result<ActiveProfile, ProfileUseCaseError> {
        let selected = {
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            persist_active_profile(&mut connection, &self.account_id, profile_id, now_utc_ms)?
        };
        let active = ActiveProfile::from(&selected);
        *self
            .active_profile
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = active.clone();
        Ok(active)
    }

    pub fn refresh_active_profile(&self, profile: &LearnerProfile) {
        let mut active = self
            .active_profile
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.id == profile.id {
            active.name.clone_from(&profile.name);
        }
    }

    pub fn database_key(&self) -> &str {
        &self.database_key
    }
}

impl From<&LearnerProfile> for ActiveProfile {
    fn from(profile: &LearnerProfile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("secure credential storage failed")]
    SecretStore(String),
    #[error("stored database key is malformed")]
    InvalidDatabaseKey,
    #[error("stored asset key is malformed")]
    InvalidAssetKey,
    #[error("stored account identity is malformed")]
    InvalidAccountId,
    #[error("stored device identity is malformed")]
    InvalidDeviceId,
    #[error("an existing library is missing required secure credentials")]
    MissingCredentials,
    #[error("local data directory could not be created")]
    File(#[from] std::io::Error),
    #[error("encrypted database could not be opened")]
    Database(#[from] DatabaseError),
    #[error("profile initialization failed")]
    Profile(#[from] ProfileUseCaseError),
    #[error("profile lookup failed")]
    Query(#[from] rusqlite::Error),
}

impl RuntimeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SecretStore(_) => "secret_store_failed",
            Self::InvalidDatabaseKey => "invalid_database_key",
            Self::InvalidAssetKey => "invalid_asset_key",
            Self::InvalidAccountId => "invalid_account_id",
            Self::InvalidDeviceId => "invalid_device_id",
            Self::MissingCredentials => "library_credentials_missing",
            Self::File(_) => "data_directory_failed",
            Self::Database(_) => "database_open_failed",
            Self::Profile(_) => "profile_initialize_failed",
            Self::Query(_) => "profile_query_failed",
        }
    }
}

pub fn initialize_local_library(
    root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<LibraryRuntime, RuntimeError> {
    let database_path = root.join("library.db");
    let existing_library = database_path.exists() || root.join("assets").exists();
    let asset_key_hex = load_required_secret(secrets, ASSET_KEY, existing_library, random_key_hex)?;
    let asset_key = decode_key(&asset_key_hex).ok_or(RuntimeError::InvalidAssetKey)?;
    let database_key =
        load_required_secret(secrets, DATABASE_KEY, existing_library, random_key_hex)?;
    if decode_key(&database_key).is_none() {
        return Err(RuntimeError::InvalidDatabaseKey);
    }
    let account_id = load_required_secret(secrets, ACCOUNT_ID, existing_library, || {
        Uuid::now_v7().to_string()
    })?;
    Uuid::parse_str(&account_id).map_err(|_| RuntimeError::InvalidAccountId)?;
    let device_id = match secrets.get(DEVICE_ID).map_err(RuntimeError::SecretStore)? {
        Some(value) => value,
        None => {
            let value = Uuid::now_v7().to_string();
            secrets
                .set(DEVICE_ID, &value)
                .map_err(RuntimeError::SecretStore)?;
            value
        }
    };
    Uuid::parse_str(&device_id).map_err(|_| RuntimeError::InvalidDeviceId)?;

    std::fs::create_dir_all(root)?;
    let mut connection = open_encrypted_database(&database_path, &database_key)?;
    run_migrations(&mut connection)?;
    let preferred_profile = connection
        .query_row(
            "SELECT p.id, p.name
             FROM account_preferences preference
             INNER JOIN learner_profiles p ON p.id = preference.active_profile_id
             WHERE preference.account_id = ?1 AND p.account_id = ?1",
            [&account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let existing_profile = connection
        .query_row(
            "SELECT id, name FROM learner_profiles WHERE account_id = ?1 ORDER BY created_at_utc_ms, id LIMIT 1",
            [&account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let (profile_id, profile_name, needs_preference) = match preferred_profile {
        Some(profile) => (profile.0, profile.1, false),
        None => match existing_profile {
            Some(profile) => (profile.0, profile.1, true),
            None => {
                let profile = create_profile(
                    &mut connection,
                    CreateProfile {
                        account_id: account_id.clone(),
                        name: "本机学习档案".to_owned(),
                        now_utc_ms,
                    },
                )?;
                (profile.id, profile.name, true)
            }
        },
    };
    if needs_preference {
        persist_active_profile(&mut connection, &account_id, &profile_id, now_utc_ms)?;
    }

    Ok(LibraryRuntime {
        connection: Arc::new(Mutex::new(connection)),
        blob_root: root.join("assets"),
        asset_key,
        database_key,
        account_id,
        device_id,
        active_profile: RwLock::new(ActiveProfile {
            id: profile_id,
            name: profile_name,
        }),
        profile_transition: Arc::new(Mutex::new(())),
    })
}

pub(crate) fn load_restore_credentials(
    secrets: &dyn SecretStore,
) -> Result<RestoreCredentials, RuntimeError> {
    let database_key = secrets
        .get(DATABASE_KEY)
        .map_err(RuntimeError::SecretStore)?
        .ok_or(RuntimeError::MissingCredentials)?;
    if decode_key(&database_key).is_none() {
        return Err(RuntimeError::InvalidDatabaseKey);
    }
    let asset_key = secrets
        .get(ASSET_KEY)
        .map_err(RuntimeError::SecretStore)?
        .ok_or(RuntimeError::MissingCredentials)?;
    let asset_key = decode_key(&asset_key).ok_or(RuntimeError::InvalidAssetKey)?;
    let account_id = secrets
        .get(ACCOUNT_ID)
        .map_err(RuntimeError::SecretStore)?
        .ok_or(RuntimeError::MissingCredentials)?;
    Uuid::parse_str(&account_id).map_err(|_| RuntimeError::InvalidAccountId)?;
    Ok(RestoreCredentials {
        database_key,
        asset_key,
        account_id,
    })
}

fn load_required_secret(
    secrets: &dyn SecretStore,
    name: &str,
    existing_library: bool,
    create: impl FnOnce() -> String,
) -> Result<String, RuntimeError> {
    if let Some(value) = secrets.get(name).map_err(RuntimeError::SecretStore)? {
        return Ok(value);
    }
    if existing_library {
        return Err(RuntimeError::MissingCredentials);
    }
    let value = create();
    secrets
        .set(name, &value)
        .map_err(RuntimeError::SecretStore)?;
    Ok(value)
}

fn random_key_hex() -> String {
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).expect("operating system random source must be available");
    let mut encoded = String::with_capacity(64);
    for byte in key {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn decode_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(key)
}
