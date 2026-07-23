use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::{
        KeyringSecretStore, SecretStore, set_library_locked, validate_library_unlock_credentials,
    },
    modules::capture_lan::CaptureLanManager,
};

const LOCAL_LIBRARY_SERVICE: &str = "com.mistaketrainer.next.local-library";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAccessStatus {
    pub locked: bool,
    pub trusted_windows_account: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartupAccessDecision {
    Unlocked,
    Locked,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibraryAccessGate {
    decision: StartupAccessDecision,
}

impl LibraryAccessGate {
    pub const fn unlocked() -> Self {
        Self {
            decision: StartupAccessDecision::Unlocked,
        }
    }

    pub const fn locked() -> Self {
        Self {
            decision: StartupAccessDecision::Locked,
        }
    }

    pub const fn unavailable() -> Self {
        Self {
            decision: StartupAccessDecision::Unavailable,
        }
    }
}

fn status(locked: bool) -> LibraryAccessStatus {
    LibraryAccessStatus {
        locked,
        trusted_windows_account: true,
    }
}

fn access_failure<T>(code: &'static str, message: &'static str) -> AppResult<T> {
    AppResult::failure(code, message, true, Uuid::now_v7().to_string())
}

pub fn access_status_for(gate: &LibraryAccessGate) -> AppResult<LibraryAccessStatus> {
    match gate.decision {
        StartupAccessDecision::Unlocked => AppResult::success(status(false)),
        StartupAccessDecision::Locked => AppResult::success(status(true)),
        StartupAccessDecision::Unavailable => access_failure(
            "LIBRARY_ACCESS_UNAVAILABLE",
            "无法读取 Windows 资料库凭据，已保持锁定；请检查系统凭据服务后重试。",
        ),
    }
}

pub fn lock_for(secrets: &dyn SecretStore) -> AppResult<LibraryAccessStatus> {
    match set_library_locked(secrets, true) {
        Ok(()) => AppResult::success(status(true)),
        Err(_) => access_failure(
            "LIBRARY_LOCK_FAILED",
            "资料库没有锁定成功，应用不会假装已经退出；请稍后重试。",
        ),
    }
}

pub fn unlock_for(secrets: &dyn SecretStore) -> AppResult<LibraryAccessStatus> {
    if validate_library_unlock_credentials(secrets).is_err() {
        return access_failure(
            "LIBRARY_UNLOCK_FAILED",
            "当前 Windows 账户无法取回完整的资料库凭据，请检查系统凭据服务后重试。",
        );
    }
    match set_library_locked(secrets, false) {
        Ok(()) => AppResult::success(status(false)),
        Err(_) => access_failure(
            "LIBRARY_UNLOCK_FAILED",
            "当前 Windows 账户无法取回资料库凭据，请检查系统凭据服务后重试。",
        ),
    }
}

fn schedule_restart(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(180)).await;
        app.restart();
    });
}

#[tauri::command]
#[specta::specta]
pub fn library_access_status(gate: State<'_, LibraryAccessGate>) -> AppResult<LibraryAccessStatus> {
    access_status_for(&gate)
}

#[tauri::command]
#[specta::specta]
pub fn library_lock(
    app: AppHandle,
    capture_lan: State<'_, CaptureLanManager>,
) -> AppResult<LibraryAccessStatus> {
    if capture_lan.stop().is_err() {
        eprintln!(
            "library lock is continuing with process restart after LAN stop reported an error"
        );
    }
    let result = lock_for(&KeyringSecretStore::new(LOCAL_LIBRARY_SERVICE));
    if matches!(result, AppResult::Success { .. }) {
        schedule_restart(app);
    }
    result
}

#[tauri::command]
#[specta::specta]
pub fn library_unlock(app: AppHandle) -> AppResult<LibraryAccessStatus> {
    let result = unlock_for(&KeyringSecretStore::new(LOCAL_LIBRARY_SERVICE));
    if matches!(result, AppResult::Success { .. }) {
        schedule_restart(app);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use crate::{application::result::AppResult, infrastructure::runtime::SecretStore};

    use super::*;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<String, String>>,
        fail_reads: bool,
        fail_writes: bool,
    }

    impl SecretStore for MemorySecretStore {
        fn get(&self, name: &str) -> Result<Option<String>, String> {
            if self.fail_reads {
                return Err("read failed".to_owned());
            }
            Ok(self
                .values
                .lock()
                .map_err(|_| "store poisoned".to_owned())?
                .get(name)
                .cloned())
        }

        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            if self.fail_writes {
                return Err("write failed".to_owned());
            }
            self.values
                .lock()
                .map_err(|_| "store poisoned".to_owned())?
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }
    }

    fn populate_library_credentials(store: &MemorySecretStore) {
        store.set("database-key", &"11".repeat(32)).unwrap();
        store.set("asset-key", &"22".repeat(32)).unwrap();
        store
            .set("account-id", "33333333-3333-4333-8333-333333333333")
            .unwrap();
    }

    #[test]
    fn access_helpers_report_and_change_the_persistent_gate() {
        let store = MemorySecretStore::default();
        populate_library_credentials(&store);
        let AppResult::Success { data, .. } = access_status_for(&LibraryAccessGate::unlocked())
        else {
            panic!("existing installations should default to unlocked")
        };
        assert!(!data.locked);
        assert!(data.trusted_windows_account);

        let AppResult::Success { data, .. } = lock_for(&store) else {
            panic!("locking should succeed")
        };
        assert!(data.locked);

        let AppResult::Success { data, .. } = unlock_for(&store) else {
            panic!("unlocking should succeed")
        };
        assert!(!data.locked);
    }

    #[test]
    fn access_helpers_fail_closed_when_credentials_are_unavailable() {
        let AppResult::Failure { error, .. } = access_status_for(&LibraryAccessGate::unavailable())
        else {
            panic!("credential read failure must not claim unlocked")
        };
        assert_eq!(error.code, "LIBRARY_ACCESS_UNAVAILABLE");

        let write_failure = MemorySecretStore {
            fail_writes: true,
            ..Default::default()
        };
        let AppResult::Failure { error, .. } = lock_for(&write_failure) else {
            panic!("credential write failure must not claim locked")
        };
        assert_eq!(error.code, "LIBRARY_LOCK_FAILED");

        let missing_credentials = MemorySecretStore::default();
        let AppResult::Failure { error, .. } = unlock_for(&missing_credentials) else {
            panic!("unlock must validate the complete credential envelope")
        };
        assert_eq!(error.code, "LIBRARY_UNLOCK_FAILED");

        let unlock_write_failure = MemorySecretStore {
            fail_writes: true,
            ..Default::default()
        };
        unlock_write_failure
            .values
            .lock()
            .unwrap()
            .insert("database-key".to_owned(), "11".repeat(32));
        unlock_write_failure
            .values
            .lock()
            .unwrap()
            .insert("asset-key".to_owned(), "22".repeat(32));
        unlock_write_failure.values.lock().unwrap().insert(
            "account-id".to_owned(),
            "33333333-3333-4333-8333-333333333333".to_owned(),
        );
        let AppResult::Failure { error, .. } = unlock_for(&unlock_write_failure) else {
            panic!("unlock marker write failure must remain locked")
        };
        assert_eq!(error.code, "LIBRARY_UNLOCK_FAILED");
    }

    #[test]
    fn startup_access_decision_never_changes_after_the_process_gate_is_created() {
        let store = MemorySecretStore::default();
        let gate = LibraryAccessGate::unlocked();
        set_library_locked(&store, true).unwrap();

        let AppResult::Success { data, .. } = access_status_for(&gate) else {
            panic!("the process gate should preserve its startup decision")
        };
        assert!(!data.locked);
    }
}
