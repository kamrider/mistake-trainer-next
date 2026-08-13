use std::fmt::Write as _;

use uuid::Uuid;

use crate::application::library_inventory::CredentialEnvelopeState;

use super::RuntimeError;

const DATABASE_KEY: &str = "database-key";
const ASSET_KEY: &str = "asset-key";
const ACCOUNT_ID: &str = "account-id";
const DEVICE_ID: &str = "device-id";
pub const LIBRARY_LOCK_STATE: &str = "library-lock-state";
const LOCAL_CREDENTIAL_NAMES: [&str; 5] = [
    DATABASE_KEY,
    ASSET_KEY,
    ACCOUNT_ID,
    DEVICE_ID,
    LIBRARY_LOCK_STATE,
];

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
    fn delete(&self, _name: &str) -> Result<(), String> {
        Err("secret deletion is not supported".to_owned())
    }
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

    fn delete(&self, name: &str) -> Result<(), String> {
        let entry = self.entry(name)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

pub(super) fn delete_local_credential_envelope(
    secrets: &dyn SecretStore,
) -> Result<(), RuntimeError> {
    for name in LOCAL_CREDENTIAL_NAMES {
        secrets.delete(name).map_err(RuntimeError::SecretStore)?;
    }
    Ok(())
}

pub(crate) struct RestoreCredentials {
    pub database_key: String,
    pub asset_key: [u8; 32],
    pub account_id: String,
}

pub(super) struct LocalCredentials {
    pub(super) database_key: String,
    pub(super) asset_key: [u8; 32],
    pub(super) account_id: String,
    pub(super) device_id: String,
}

pub(super) fn library_is_locked(secrets: &dyn SecretStore) -> Result<bool, RuntimeError> {
    match secrets
        .get(LIBRARY_LOCK_STATE)
        .map_err(RuntimeError::SecretStore)?
        .as_deref()
    {
        None | Some("unlocked") => Ok(false),
        Some("locked") => Ok(true),
        Some(_) => Err(RuntimeError::InvalidLibraryLockState),
    }
}

pub(super) fn inspect_local_credential_envelope(
    secrets: &dyn SecretStore,
) -> Result<CredentialEnvelopeState, RuntimeError> {
    let database_key = secrets
        .get(DATABASE_KEY)
        .map_err(RuntimeError::SecretStore)?;
    let asset_key = secrets.get(ASSET_KEY).map_err(RuntimeError::SecretStore)?;
    let account_id = secrets.get(ACCOUNT_ID).map_err(RuntimeError::SecretStore)?;
    let device_id = secrets.get(DEVICE_ID).map_err(RuntimeError::SecretStore)?;
    let lock_state = secrets
        .get(LIBRARY_LOCK_STATE)
        .map_err(RuntimeError::SecretStore)?;

    if database_key.is_none()
        && asset_key.is_none()
        && account_id.is_none()
        && device_id.is_none()
        && lock_state.is_none()
    {
        return Ok(CredentialEnvelopeState::Absent);
    }

    let core_is_valid = database_key.as_deref().and_then(decode_key).is_some()
        && asset_key.as_deref().and_then(decode_key).is_some()
        && account_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some();
    let device_is_valid = device_id
        .as_deref()
        .map(|value| Uuid::parse_str(value).is_ok())
        .unwrap_or(true);
    let lock_is_valid = matches!(lock_state.as_deref(), None | Some("locked" | "unlocked"));

    Ok(if core_is_valid && device_is_valid && lock_is_valid {
        CredentialEnvelopeState::Complete
    } else {
        CredentialEnvelopeState::Partial
    })
}

pub(super) fn set_library_locked(
    secrets: &dyn SecretStore,
    locked: bool,
) -> Result<(), RuntimeError> {
    secrets
        .set(
            LIBRARY_LOCK_STATE,
            if locked { "locked" } else { "unlocked" },
        )
        .map_err(RuntimeError::SecretStore)
}

pub(super) fn load_or_create_local_credentials(
    secrets: &dyn SecretStore,
    existing_library: bool,
) -> Result<LocalCredentials, RuntimeError> {
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

    Ok(LocalCredentials {
        database_key,
        asset_key,
        account_id,
        device_id,
    })
}

pub(super) fn load_restore_credentials(
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
