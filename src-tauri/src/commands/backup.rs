use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    application::{library_inventory::LibraryRecoveryReason, result::AppResult},
    commands::{
        access::{LibraryAccessGate, recovery_reason_for},
        storage::ApplicationControlRoot,
    },
    infrastructure::runtime::{KeyringSecretStore, LibraryRuntime, load_restore_credentials},
    modules::{
        automatic_backup::{AutomaticBackupPolicyCoordinator, AutomaticBackupStatus},
        backup::{
            BackupError, BackupRestoreCandidate, BackupRestoreReceipt, BackupSummary,
            PortableBackupReceipt, RestoreMode, create_backup, create_portable_backup,
            prepare_backup_restore, prepare_portable_backup_restore, schedule_backup_restore,
            schedule_backup_restore_with_mode, take_restore_receipt,
        },
        capture_lan::CaptureLanManager,
    },
};

const LOCAL_LIBRARY_SERVICE: &str = "com.mistaketrainer.next.local-library";

#[tauri::command]
#[specta::specta]
pub fn backup_automatic_status(
    control_root: State<'_, ApplicationControlRoot>,
    coordinator: State<'_, AutomaticBackupPolicyCoordinator>,
) -> AppResult<AutomaticBackupStatus> {
    match coordinator.status(&control_root.0) {
        Ok(status) => AppResult::success(status),
        Err(error) => backup_failure("backup_automatic_status_failed", &error),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn backup_automatic_configure(
    control_root: State<'_, ApplicationControlRoot>,
    coordinator: State<'_, AutomaticBackupPolicyCoordinator>,
    interval_days: u32,
    retention_count: u32,
) -> Result<AppResult<Option<AutomaticBackupStatus>>, ()> {
    let control_root = control_root.0.clone();
    let coordinator = coordinator.inner().clone();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let Some(destination) = rfd::FileDialog::new()
            .set_title("选择自动备份文件夹")
            .pick_folder()
        else {
            return Ok(None);
        };
        coordinator
            .configure(&control_root, &destination, interval_days, retention_count)
            .map(Some)
    });
    Ok(match worker.await {
        Ok(Ok(status)) => AppResult::success(status),
        Ok(Err(error)) => backup_failure("backup_automatic_configure_failed", &error),
        Err(_) => backup_failure("backup_automatic_configure_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub fn backup_automatic_disable(
    control_root: State<'_, ApplicationControlRoot>,
    coordinator: State<'_, AutomaticBackupPolicyCoordinator>,
) -> AppResult<AutomaticBackupStatus> {
    match coordinator.disable(&control_root.0) {
        Ok(status) => AppResult::success(status),
        Err(error) => backup_failure("backup_automatic_disable_failed", &error),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn backup_create(
    state: State<'_, LibraryRuntime>,
) -> Result<AppResult<Option<BackupSummary>>, ()> {
    let connection = Arc::clone(&state.connection);
    let blob_root = state.blob_root.clone();
    let database_key = state.database_key().to_owned();
    let account_id = state.account_id().to_owned();
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let destination = rfd::FileDialog::new()
            .set_title("选择加密备份保存位置")
            .pick_folder();
        create_for_selected_destination(
            &connection,
            &blob_root,
            &database_key,
            &account_id,
            destination,
        )
    });
    Ok(match worker.await {
        Ok(Ok(summary)) => AppResult::success(summary),
        Ok(Err(error)) => backup_failure("backup_create_failed", &error),
        Err(_) => backup_failure("backup_create_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_create_portable(
    state: State<'_, LibraryRuntime>,
) -> Result<AppResult<Option<PortableBackupReceipt>>, ()> {
    let connection = Arc::clone(&state.connection);
    let blob_root = state.blob_root.clone();
    let database_key = state.database_key().to_owned();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let destination = rfd::FileDialog::new()
            .set_title("选择便携加密备份保存位置")
            .pick_folder();
        create_portable_for_selected_destination(
            &connection,
            &blob_root,
            &database_key,
            &asset_key,
            &account_id,
            destination,
        )
    });
    Ok(match worker.await {
        Ok(Ok(receipt)) => AppResult::success(receipt),
        Ok(Err(error)) => backup_failure("backup_create_portable_failed", &error),
        Err(_) => backup_failure("backup_create_portable_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_prepare_restore(
    state: State<'_, LibraryRuntime>,
) -> Result<AppResult<Option<BackupRestoreCandidate>>, ()> {
    let database_key = state.database_key().to_owned();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    let application_root = application_root(&state.blob_root);
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let source = rfd::FileDialog::new()
            .set_title("选择要恢复的 Mistake Trainer 加密备份目录")
            .pick_folder();
        prepare_selected_package(
            application_root?,
            &database_key,
            &asset_key,
            &account_id,
            source,
        )
    });
    Ok(match worker.await {
        Ok(Ok(summary)) => AppResult::success(summary),
        Ok(Err(error)) => backup_failure("backup_prepare_restore_failed", &error),
        Err(_) => backup_failure("backup_prepare_restore_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_prepare_portable_restore(
    state: State<'_, LibraryRuntime>,
    recovery_key: String,
) -> Result<AppResult<Option<BackupRestoreCandidate>>, ()> {
    let database_key = state.database_key().to_owned();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    let application_root = application_root(&state.blob_root);
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let source = rfd::FileDialog::new()
            .set_title("选择要跨设备恢复的 Mistake Trainer 便携备份目录")
            .pick_folder();
        prepare_selected_portable_package(
            application_root?,
            &recovery_key,
            &database_key,
            &asset_key,
            &account_id,
            source,
        )
    });
    Ok(match worker.await {
        Ok(Ok(candidate)) => AppResult::success(candidate),
        Ok(Err(error)) => backup_failure("backup_prepare_portable_restore_failed", &error),
        Err(_) => backup_failure("backup_prepare_portable_restore_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_recovery_prepare(
    gate: State<'_, LibraryAccessGate>,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<Option<BackupRestoreCandidate>>, ()> {
    if recovery_reason_for(&gate) != Some(LibraryRecoveryReason::LocalDataMissing) {
        return Ok(AppResult::failure(
            "backup_recovery_not_allowed",
            "当前状态不允许从备份引导恢复。",
            false,
            Uuid::now_v7().to_string(),
        ));
    }
    let application_root = control_root.0.clone();
    let credentials =
        match load_restore_credentials(&KeyringSecretStore::new(LOCAL_LIBRARY_SERVICE)) {
            Ok(credentials) => credentials,
            Err(_) => {
                return Ok(AppResult::failure(
                    "backup_recovery_credentials_failed",
                    "无法读取当前 Windows 账户的资料库恢复凭据，请稍后重试。",
                    true,
                    Uuid::now_v7().to_string(),
                ));
            }
        };
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let source = rfd::FileDialog::new()
            .set_title("选择要恢复的 Mistake Trainer 加密备份目录")
            .pick_folder();
        prepare_selected_package(
            application_root,
            &credentials.database_key,
            &credentials.asset_key,
            &credentials.account_id,
            source,
        )
    });
    Ok(match worker.await {
        Ok(Ok(candidate)) => AppResult::success(candidate),
        Ok(Err(error)) => backup_failure("backup_recovery_prepare_failed", &error),
        Err(_) => backup_failure("backup_recovery_prepare_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_recovery_restore(
    app: AppHandle,
    gate: State<'_, LibraryAccessGate>,
    control_root: State<'_, ApplicationControlRoot>,
    candidate_id: String,
) -> Result<AppResult<bool>, ()> {
    if recovery_reason_for(&gate) != Some(LibraryRecoveryReason::LocalDataMissing) {
        return Ok(AppResult::failure(
            "backup_recovery_not_allowed",
            "当前状态不允许从备份引导恢复。",
            false,
            Uuid::now_v7().to_string(),
        ));
    }
    let application_root = control_root.0.clone();
    let credentials =
        match load_restore_credentials(&KeyringSecretStore::new(LOCAL_LIBRARY_SERVICE)) {
            Ok(credentials) => credentials,
            Err(_) => {
                return Ok(AppResult::failure(
                    "backup_recovery_credentials_failed",
                    "无法读取当前 Windows 账户的资料库恢复凭据，请稍后重试。",
                    true,
                    Uuid::now_v7().to_string(),
                ));
            }
        };
    let worker = tauri::async_runtime::spawn_blocking(move || {
        schedule_backup_restore_with_mode(
            &application_root,
            &candidate_id,
            &credentials.database_key,
            &credentials.asset_key,
            &credentials.account_id,
            current_utc_millis(),
            RestoreMode::BootstrapMissing,
        )
    });
    Ok(match worker.await {
        Ok(Ok(_)) => schedule_restore_restart(app),
        Ok(Err(error)) => backup_failure("backup_recovery_restore_failed", &error),
        Err(_) => backup_failure("backup_recovery_restore_failed", &BackupError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    lan: State<'_, CaptureLanManager>,
    candidate_id: String,
) -> Result<AppResult<bool>, ()> {
    if lan.stop().is_err() {
        return Ok(AppResult::failure(
            "backup_restore_capture_stop_failed",
            "手机采集服务暂时无法停止，请稍后重试。现有资料库没有变化。",
            true,
            Uuid::now_v7().to_string(),
        ));
    }
    let database_key = state.database_key().to_owned();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    let application_root = application_root(&state.blob_root);
    drop(state);
    drop(lan);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        schedule_backup_restore(
            &application_root?,
            &candidate_id,
            &database_key,
            &asset_key,
            &account_id,
            current_utc_millis(),
        )
    });
    Ok(match worker.await {
        Ok(Ok(_)) => schedule_restore_restart(app),
        Ok(Err(error)) => backup_failure("backup_restore_failed", &error),
        Err(_) => backup_failure("backup_restore_failed", &BackupError::Lock),
    })
}

fn schedule_restore_restart(app: AppHandle) -> AppResult<bool> {
    let restart = std::thread::Builder::new()
        .name("mistake-trainer-restore-restart".to_owned())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(450));
            app.restart();
        });
    match restart {
        Ok(_) => AppResult::success(true),
        Err(_) => AppResult::failure(
            "backup_restore_restart_failed",
            "恢复任务已安全安排，但自动重启没有启动。请关闭并重新打开应用，恢复会在下次启动时继续。",
            false,
            Uuid::now_v7().to_string(),
        ),
    }
}

#[tauri::command]
#[specta::specta]
pub fn backup_restore_status(
    state: State<'_, LibraryRuntime>,
) -> AppResult<Option<BackupRestoreReceipt>> {
    let result = application_root(&state.blob_root).and_then(|root| take_restore_receipt(&root));
    match result {
        Ok(receipt) => AppResult::success(receipt),
        Err(error) => backup_failure("backup_restore_status_failed", &error),
    }
}

fn create_for_selected_destination(
    connection: &Mutex<Connection>,
    blob_root: &std::path::Path,
    database_key: &str,
    account_id: &str,
    destination: Option<PathBuf>,
) -> Result<Option<BackupSummary>, BackupError> {
    let Some(destination) = destination else {
        return Ok(None);
    };
    create_backup(
        connection,
        blob_root,
        database_key,
        account_id,
        &destination,
        current_utc_millis(),
    )
    .map(Some)
}

fn create_portable_for_selected_destination(
    connection: &Mutex<Connection>,
    blob_root: &std::path::Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    destination: Option<PathBuf>,
) -> Result<Option<PortableBackupReceipt>, BackupError> {
    let Some(destination) = destination else {
        return Ok(None);
    };
    create_portable_backup(
        connection,
        blob_root,
        database_key,
        asset_key,
        account_id,
        &destination,
        current_utc_millis(),
    )
    .map(Some)
}

fn prepare_selected_package(
    application_root: PathBuf,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    source: Option<PathBuf>,
) -> Result<Option<BackupRestoreCandidate>, BackupError> {
    let Some(source) = source else {
        return Ok(None);
    };
    prepare_backup_restore(
        &source,
        &application_root,
        database_key,
        asset_key,
        account_id,
        current_utc_millis(),
    )
    .map(Some)
}

fn prepare_selected_portable_package(
    application_root: PathBuf,
    recovery_key: &str,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    source: Option<PathBuf>,
) -> Result<Option<BackupRestoreCandidate>, BackupError> {
    let Some(source) = source else {
        return Ok(None);
    };
    prepare_portable_backup_restore(
        &source,
        &application_root,
        recovery_key.trim(),
        database_key,
        asset_key,
        account_id,
        current_utc_millis(),
    )
    .map(Some)
}

fn application_root(blob_root: &std::path::Path) -> Result<PathBuf, BackupError> {
    blob_root
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .ok_or(BackupError::InvalidDestination)
}

fn backup_failure<T>(code: &str, error: &BackupError) -> AppResult<T> {
    let (user_message, retryable) = match error {
        BackupError::AccountMismatch => (
            "这个备份不属于当前本机学习账户，未对现有资料库做任何修改。",
            false,
        ),
        BackupError::ForeignAccountData => (
            "本机资料库包含其他账户的数据，已停止备份或恢复验证，请先使用诊断工具检查账户隔离。",
            false,
        ),
        BackupError::UnsupportedSchema => (
            "这个备份来自更新版本的应用，请先升级 Mistake Trainer 后再验证。",
            false,
        ),
        BackupError::InvalidPackage | BackupError::Integrity => {
            ("备份包不完整或校验失败，未对现有资料库做任何修改。", false)
        }
        BackupError::InvalidRecoveryKey => (
            "恢复密钥不正确，或便携备份已经被修改；现有资料库没有改变。",
            false,
        ),
        BackupError::Crypto => (
            "无法安全创建便携备份，请稍后重试；现有资料库没有改变。",
            true,
        ),
        BackupError::InvalidPolicy => (
            "自动备份设置无效；间隔应为 1–30 天，保留数量应为 1–20 份。",
            false,
        ),
        BackupError::ExpiredCandidate => {
            ("这个恢复包的安全暂存已过期，请重新选择并验证备份。", false)
        }
        BackupError::RestorePending => (
            "已有一个恢复任务等待应用重启，请先重新打开应用完成它。",
            false,
        ),
        BackupError::TooLarge => ("备份超出当前版本的安全处理上限，未继续读取或写入。", false),
        BackupError::InvalidDestination => {
            ("该位置不能用于保存备份，请选择资料库之外的文件夹。", true)
        }
        BackupError::Lock | BackupError::Io(_) | BackupError::Database(_) => (
            "备份操作没有完成，现有资料库未被替换，请检查磁盘空间后重试。",
            true,
        ),
    };
    AppResult::failure(code, user_message, retryable, Uuid::now_v7().to_string())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        backup_failure, create_portable_for_selected_destination, prepare_selected_package,
    };
    use crate::modules::backup::BackupError;

    #[test]
    fn cancelling_package_selection_returns_no_summary() {
        let result = prepare_selected_package(
            std::path::PathBuf::from("unused"),
            "unused",
            &[0_u8; 32],
            "unused",
            None,
        )
        .expect("cancel succeeds");
        assert_eq!(result, None);
    }

    #[test]
    fn cancelling_portable_destination_selection_does_not_create_a_recovery_key() {
        let connection = std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap());
        let result = create_portable_for_selected_destination(
            &connection,
            std::path::Path::new("unused"),
            "unused",
            &[0_u8; 32],
            "unused",
            None,
        )
        .expect("cancel succeeds");
        assert_eq!(result, None);
    }

    #[test]
    fn public_failures_never_serialize_internal_paths() {
        let result: crate::application::result::AppResult<()> = backup_failure(
            "backup_prepare_restore_failed",
            &BackupError::Io(std::io::Error::other("C:\\Users\\private\\backup")),
        );
        let serialized = serde_json::to_string(&result).expect("serialize AppResult");
        assert!(!serialized.contains("Users"));
        assert!(!serialized.contains("private"));
        assert!(serialized.contains("现有资料库未被替换"));
    }
}
