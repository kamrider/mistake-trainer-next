use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    application::{
        library_inventory::{LibraryArtifactState, LibraryRecoveryReason},
        result::AppResult,
    },
    commands::storage::ApplicationControlRoot,
    infrastructure::library_inventory::inspect_library_artifacts,
    infrastructure::library_reset::reset_missing_library,
    infrastructure::runtime::{
        KeyringSecretStore, SecretStore, set_library_locked, validate_existing_library,
        validate_library_unlock_credentials,
    },
    infrastructure::storage_location::{
        RESET_PENDING_FILE, RESTORE_PENDING_FILE, STORAGE_PENDING_FILE, control_file_present,
        resolve_storage,
    },
    modules::capture_lan::CaptureLanManager,
};

const LOCAL_LIBRARY_SERVICE: &str = "com.mistaketrainer.next.local-library";
const FRESH_START_CONFIRMATION: &str = "永久放弃原资料库";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAccessStatus {
    pub state: LibraryAccessState,
    pub trusted_windows_account: bool,
    pub recovery_reason: Option<LibraryRecoveryReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LibraryAccessState {
    Unlocked,
    Locked,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartupAccessDecision {
    Unlocked,
    Locked,
    CredentialsUnavailable,
    RecoveryRequired(LibraryRecoveryReason),
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
            decision: StartupAccessDecision::CredentialsUnavailable,
        }
    }

    pub const fn storage_unavailable() -> Self {
        Self::recovery(LibraryRecoveryReason::StorageDisconnected)
    }

    pub const fn recovery(reason: LibraryRecoveryReason) -> Self {
        Self {
            decision: StartupAccessDecision::RecoveryRequired(reason),
        }
    }
}

fn status(state: LibraryAccessState) -> LibraryAccessStatus {
    LibraryAccessStatus {
        state,
        trusted_windows_account: true,
        recovery_reason: None,
    }
}

fn recovery_status(reason: LibraryRecoveryReason) -> LibraryAccessStatus {
    LibraryAccessStatus {
        state: LibraryAccessState::RecoveryRequired,
        trusted_windows_account: !matches!(
            reason,
            LibraryRecoveryReason::SetupInterrupted
                | LibraryRecoveryReason::CredentialsIncomplete
                | LibraryRecoveryReason::ResetIncomplete
        ),
        recovery_reason: Some(reason),
    }
}

fn access_failure<T>(code: &'static str, message: &'static str) -> AppResult<T> {
    AppResult::failure(code, message, true, Uuid::now_v7().to_string())
}

pub fn access_status_for(gate: &LibraryAccessGate) -> AppResult<LibraryAccessStatus> {
    match gate.decision {
        StartupAccessDecision::Unlocked => AppResult::success(status(LibraryAccessState::Unlocked)),
        StartupAccessDecision::Locked => AppResult::success(status(LibraryAccessState::Locked)),
        StartupAccessDecision::CredentialsUnavailable => access_failure(
            "LIBRARY_ACCESS_UNAVAILABLE",
            "无法读取 Windows 资料库凭据，已保持锁定；请检查系统凭据服务后重试。",
        ),
        StartupAccessDecision::RecoveryRequired(reason) => {
            AppResult::success(recovery_status(reason))
        }
    }
}

pub const fn recovery_reason_for(gate: &LibraryAccessGate) -> Option<LibraryRecoveryReason> {
    match gate.decision {
        StartupAccessDecision::RecoveryRequired(reason) => Some(reason),
        _ => None,
    }
}

pub fn lock_for(secrets: &dyn SecretStore) -> AppResult<LibraryAccessStatus> {
    match set_library_locked(secrets, true) {
        Ok(()) => AppResult::success(status(LibraryAccessState::Locked)),
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
        Ok(()) => AppResult::success(status(LibraryAccessState::Unlocked)),
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

pub fn retry_for(gate: &LibraryAccessGate) -> AppResult<bool> {
    if matches!(
        gate.decision,
        StartupAccessDecision::CredentialsUnavailable | StartupAccessDecision::RecoveryRequired(_)
    ) {
        AppResult::success(true)
    } else {
        AppResult::failure(
            "LIBRARY_RECOVERY_NOT_REQUIRED",
            "当前资料库不需要恢复检查。",
            false,
            Uuid::now_v7().to_string(),
        )
    }
}

pub fn start_fresh_for(
    gate: &LibraryAccessGate,
    control_root: &std::path::Path,
    secrets: &dyn SecretStore,
    confirmation: &str,
) -> AppResult<bool> {
    if confirmation != FRESH_START_CONFIRMATION {
        return access_failure(
            "LIBRARY_FRESH_START_CONFIRMATION_REQUIRED",
            "确认文字不匹配；没有更改资料库。",
        );
    }
    if !matches!(
        gate.decision,
        StartupAccessDecision::RecoveryRequired(
            LibraryRecoveryReason::LocalDataMissing
                | LibraryRecoveryReason::SetupInterrupted
                | LibraryRecoveryReason::ResetIncomplete
        )
    ) {
        return access_failure(
            "LIBRARY_FRESH_START_NOT_ALLOWED",
            "当前状态不允许放弃原资料库。",
        );
    }
    if fresh_start_preflight(control_root, secrets).is_err() {
        return access_failure(
            "LIBRARY_FRESH_START_EVIDENCE_CHANGED",
            "资料库状态已经变化，或仍有待完成的恢复操作；没有删除任何凭据。请重新启动应用检查。",
        );
    }
    match reset_missing_library(control_root, secrets) {
        Ok(()) => AppResult::success(true),
        Err(error) => AppResult::failure(
            "LIBRARY_FRESH_START_FAILED",
            "重新开始尚未完成；应用保留了恢复标记，可以安全重试。",
            true,
            format!("{}-{}", error.code(), Uuid::now_v7()),
        ),
    }
}

fn fresh_start_preflight(
    control_root: &std::path::Path,
    secrets: &dyn SecretStore,
) -> Result<(), ()> {
    // ResetIncomplete is an allowed continuation, but every other operation
    // must be proven absent immediately before the first destructive write.
    control_file_present(control_root, RESET_PENDING_FILE).map_err(|_| ())?;
    if control_file_present(control_root, STORAGE_PENDING_FILE).map_err(|_| ())? {
        return Err(());
    }

    let storage = resolve_storage(control_root).map_err(|_| ())?;
    let library_root = storage.library_root();
    let application_root = library_root.parent().ok_or(())?;
    if control_file_present(application_root, RESTORE_PENDING_FILE).map_err(|_| ())? {
        return Err(());
    }

    if validate_existing_library(library_root, secrets).is_ok() {
        return Err(());
    }
    match inspect_library_artifacts(library_root).map_err(|_| ())? {
        LibraryArtifactState::Absent => Ok(()),
        LibraryArtifactState::Present => Err(()),
    }
}

#[tauri::command]
#[specta::specta]
pub fn library_access_retry(app: AppHandle, gate: State<'_, LibraryAccessGate>) -> AppResult<bool> {
    let result = retry_for(&gate);
    if matches!(result, AppResult::Success { .. }) {
        schedule_restart(app);
    }
    result
}

#[tauri::command]
#[specta::specta]
pub fn library_recovery_start_fresh(
    app: AppHandle,
    gate: State<'_, LibraryAccessGate>,
    control_root: State<'_, ApplicationControlRoot>,
    confirmation: String,
) -> AppResult<bool> {
    let result = start_fresh_for(
        &gate,
        &control_root.0,
        &KeyringSecretStore::new(LOCAL_LIBRARY_SERVICE),
        &confirmation,
    );
    if matches!(result, AppResult::Success { .. }) {
        schedule_restart(app);
    }
    result
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

        fn delete(&self, name: &str) -> Result<(), String> {
            self.values
                .lock()
                .map_err(|_| "store poisoned".to_owned())?
                .remove(name);
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
        assert_eq!(data.state, LibraryAccessState::Unlocked);
        assert!(data.trusted_windows_account);

        let AppResult::Success { data, .. } = lock_for(&store) else {
            panic!("locking should succeed")
        };
        assert_eq!(data.state, LibraryAccessState::Locked);

        let AppResult::Success { data, .. } = unlock_for(&store) else {
            panic!("unlocking should succeed")
        };
        assert_eq!(data.state, LibraryAccessState::Unlocked);
    }

    #[test]
    fn access_helpers_fail_closed_when_credentials_are_unavailable() {
        let AppResult::Failure { error, .. } = access_status_for(&LibraryAccessGate::unavailable())
        else {
            panic!("credential read failure must not claim unlocked")
        };
        assert_eq!(error.code, "LIBRARY_ACCESS_UNAVAILABLE");

        let AppResult::Success { data, .. } =
            access_status_for(&LibraryAccessGate::storage_unavailable())
        else {
            panic!("known storage recovery is a structured status")
        };
        assert_eq!(data.state, LibraryAccessState::RecoveryRequired);
        assert_eq!(
            data.recovery_reason,
            Some(LibraryRecoveryReason::StorageDisconnected)
        );
        assert!(matches!(
            retry_for(&LibraryAccessGate::storage_unavailable()),
            AppResult::Success { data: true, .. }
        ));

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
        assert_eq!(data.state, LibraryAccessState::Unlocked);
    }

    #[test]
    fn fresh_start_requires_exact_confirmation_and_a_permitted_recovery_gate() {
        let root = tempfile::tempdir().unwrap();
        let store = MemorySecretStore::default();
        populate_library_credentials(&store);
        let gate = LibraryAccessGate::recovery(LibraryRecoveryReason::LocalDataMissing);

        assert!(matches!(
            start_fresh_for(&gate, root.path(), &store, "放弃"),
            AppResult::Failure { .. }
        ));
        assert!(store.get("database-key").unwrap().is_some());
        assert!(matches!(
            start_fresh_for(
                &LibraryAccessGate::storage_unavailable(),
                root.path(),
                &store,
                FRESH_START_CONFIRMATION
            ),
            AppResult::Failure { .. }
        ));
        std::fs::create_dir_all(root.path().join("library")).unwrap();
        std::fs::write(root.path().join("library/library.db"), b"reappeared").unwrap();
        let AppResult::Failure { error, .. } =
            start_fresh_for(&gate, root.path(), &store, FRESH_START_CONFIRMATION)
        else {
            panic!("fresh evidence must veto deletion when library data reappears")
        };
        assert_eq!(error.code, "LIBRARY_FRESH_START_EVIDENCE_CHANGED");
        assert!(store.get("database-key").unwrap().is_some());
        std::fs::remove_dir_all(root.path().join("library")).unwrap();
        assert!(matches!(
            start_fresh_for(&gate, root.path(), &store, FRESH_START_CONFIRMATION),
            AppResult::Success { data: true, .. }
        ));
        assert!(store.get("database-key").unwrap().is_none());
    }
}
