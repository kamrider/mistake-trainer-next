use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;

use crate::{
    infrastructure::database::{DatabaseError, open_encrypted_database, run_migrations},
    modules::profiles::{
        CreateProfile, LearnerProfile, ProfileUseCaseError, create_profile, persist_active_profile,
    },
};

#[path = "runtime_credentials.rs"]
mod credentials;

pub use credentials::LIBRARY_LOCK_STATE;
pub(crate) use credentials::RestoreCredentials;
pub use credentials::{KeyringSecretStore, SecretStore};

pub fn library_is_locked(secrets: &dyn SecretStore) -> Result<bool, RuntimeError> {
    credentials::library_is_locked(secrets)
}

pub fn set_library_locked(secrets: &dyn SecretStore, locked: bool) -> Result<(), RuntimeError> {
    credentials::set_library_locked(secrets, locked)
}

pub fn validate_library_unlock_credentials(secrets: &dyn SecretStore) -> Result<(), RuntimeError> {
    credentials::load_restore_credentials(secrets).map(|_| ())
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

    pub fn replace_active_profile(&self, profile: &LearnerProfile) {
        *self
            .active_profile
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = ActiveProfile::from(profile);
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
    #[error("stored library lock state is malformed")]
    InvalidLibraryLockState,
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
            Self::InvalidLibraryLockState => "invalid_library_lock_state",
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
    let credentials::LocalCredentials {
        database_key,
        asset_key,
        account_id,
        device_id,
    } = credentials::load_or_create_local_credentials(secrets, existing_library)?;

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
    credentials::load_restore_credentials(secrets)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn get(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self
                .values
                .lock()
                .map_err(|_| "secret store poisoned".to_owned())?
                .get(name)
                .cloned())
        }

        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            self.values
                .lock()
                .map_err(|_| "secret store poisoned".to_owned())?
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }
    }

    #[test]
    fn library_lock_marker_is_strict() {
        let store = MemorySecretStore::default();

        assert!(!library_is_locked(&store).expect("missing marker is unlocked"));
        set_library_locked(&store, true).expect("lock marker");
        assert!(library_is_locked(&store).expect("locked marker"));
        set_library_locked(&store, false).expect("unlock marker");
        assert!(!library_is_locked(&store).expect("unlocked marker"));

        store
            .set(LIBRARY_LOCK_STATE, "corrupt")
            .expect("write malformed marker");
        assert!(matches!(
            library_is_locked(&store),
            Err(RuntimeError::InvalidLibraryLockState)
        ));
    }
}
