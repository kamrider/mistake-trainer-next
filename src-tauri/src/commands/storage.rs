use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::{
        runtime::LibraryRuntime,
        storage_location::{ResolvedStorage, resolve_storage},
    },
    modules::{
        capture_lan::{CaptureLanError, CaptureLanManager},
        storage_migration::{
            StorageMigrationError, StorageMigrationReceipt, StorageMigrationSource,
            read_storage_migration_receipt, stage_storage_migration_from_source,
            storage_migration_pending, storage_usage_bytes,
        },
    },
};

const RESTART_DELAY: Duration = Duration::from_millis(450);

pub struct ApplicationControlRoot(pub PathBuf);

impl std::fmt::Debug for ApplicationControlRoot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ApplicationControlRoot")
            .field(&"<fixed application data>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StorageLocationKind {
    Default,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocationStatus {
    pub kind: StorageLocationKind,
    pub location_label: String,
    pub database_bytes: f64,
    pub asset_bytes: f64,
    pub migration_pending: bool,
}

pub fn storage_status_for(
    runtime: &LibraryRuntime,
    control_root: &Path,
) -> AppResult<StorageLocationStatus> {
    storage_status_from_source(&StorageMigrationSource::from_runtime(runtime), control_root)
}

fn storage_status_from_source(
    source: &StorageMigrationSource,
    control_root: &Path,
) -> AppResult<StorageLocationStatus> {
    let result = (|| {
        let resolved = resolve_storage(control_root)?;
        let current_root = source
            .library_root()
            .ok_or(StorageMigrationError::InvalidDestination)?
            .canonicalize()
            .map_err(|_| StorageMigrationError::InvalidDestination)?;
        let configured_root = resolved
            .library_root()
            .canonicalize()
            .map_err(|_| StorageMigrationError::InvalidDestination)?;
        if current_root != configured_root {
            return Err(StorageMigrationError::InvalidDestination);
        }
        let (database_bytes, asset_bytes) = storage_usage_bytes(source)?;
        Ok(StorageLocationStatus {
            kind: if resolved.is_custom() {
                StorageLocationKind::Custom
            } else {
                StorageLocationKind::Default
            },
            location_label: redacted_location_label(&resolved),
            database_bytes: database_bytes as f64,
            asset_bytes: asset_bytes as f64,
            migration_pending: storage_migration_pending(control_root)?,
        })
    })();

    match result {
        Ok(status) => AppResult::success(status),
        Err(error) => storage_status_failure(&error),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn storage_status(
    runtime: State<'_, LibraryRuntime>,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<StorageLocationStatus>, ()> {
    let source = StorageMigrationSource::from_runtime(&runtime);
    let control_root = control_root.0.clone();
    drop(runtime);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        storage_status_from_source(&source, &control_root)
    });
    Ok(match worker.await {
        Ok(result) => result,
        Err(_) => storage_status_failure(&StorageMigrationError::Lock),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn storage_migrate_select(
    app: AppHandle,
    runtime: State<'_, LibraryRuntime>,
    control_root: State<'_, ApplicationControlRoot>,
    lan: State<'_, CaptureLanManager>,
) -> Result<AppResult<Option<StorageMigrationReceipt>>, ()> {
    let source = StorageMigrationSource::from_runtime(&runtime);
    let control_root = control_root.0.clone();
    let lan = lan.inner().clone();
    drop(runtime);

    let selected = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择新的资料库存储位置")
            .pick_folder()
    })
    .await;
    let selected = match selected {
        Ok(selected) => match require_selected_destination(selected) {
            Ok(selected) => selected,
            Err(cancelled) => return Ok(cancelled),
        },
        Err(_) => {
            return Ok(AppResult::failure(
                "storage_dialog_failed",
                "文件夹选择窗口没有正常打开，请稍后重试。",
                true,
                Uuid::now_v7().to_string(),
            ));
        }
    };

    if let Err(error) = lan.stop() {
        return Ok(capture_stop_failure(&error));
    }
    let staged = tauri::async_runtime::spawn_blocking(move || {
        stage_storage_migration_from_source(&source, &control_root, &selected, current_utc_millis())
    })
    .await;

    Ok(match staged {
        Ok(Ok(receipt)) => {
            schedule_restart(app);
            AppResult::success(Some(receipt))
        }
        Ok(Err(error)) => storage_migration_failure(&error),
        Err(_) => storage_migration_failure(&StorageMigrationError::Lock),
    })
}

fn require_selected_destination(
    selected: Option<PathBuf>,
) -> Result<PathBuf, AppResult<Option<StorageMigrationReceipt>>> {
    selected.ok_or_else(|| AppResult::success(None))
}

#[tauri::command]
#[specta::specta]
pub fn storage_migration_receipt(
    control_root: State<'_, ApplicationControlRoot>,
) -> AppResult<Option<StorageMigrationReceipt>> {
    match read_storage_migration_receipt(&control_root.0) {
        Ok(receipt) => AppResult::success(receipt),
        Err(error) => AppResult::failure(
            "storage_migration_receipt_failed",
            "无法读取上一次存储迁移的结果，请稍后重试。",
            error_is_retryable(&error),
            Uuid::now_v7().to_string(),
        ),
    }
}

fn schedule_restart(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(RESTART_DELAY).await;
        app.restart();
    });
}

fn redacted_location_label(storage: &ResolvedStorage) -> String {
    if !storage.is_custom() {
        return "默认位置 · Windows 应用数据".to_owned();
    }
    storage
        .library_root()
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("自定义位置 · {value}"))
        .unwrap_or_else(|| "自定义位置".to_owned())
}

fn capture_stop_failure<T>(error: &CaptureLanError) -> AppResult<T> {
    eprintln!("storage migration stopped before snapshot [{}]", error);
    AppResult::failure(
        "storage_capture_stop_failed",
        "手机采集服务暂时无法安全停止，迁移尚未开始；请结束采集后重试。",
        true,
        Uuid::now_v7().to_string(),
    )
}

fn storage_status_failure<T>(error: &StorageMigrationError) -> AppResult<T> {
    eprintln!("storage status failed [{}]", error.code());
    AppResult::failure(
        "storage_status_failed",
        "无法读取当前资料库的容量信息，请稍后重试。",
        error_is_retryable(error),
        Uuid::now_v7().to_string(),
    )
}

fn storage_migration_failure<T>(error: &StorageMigrationError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        StorageMigrationError::InvalidDestination => (
            "storage_destination_invalid",
            "请选择资料库之外的空文件夹；现有资料没有变化。",
            true,
        ),
        StorageMigrationError::DestinationInUse => (
            "storage_destination_in_use",
            "所选位置已有其他文件，未写入任何资料。",
            true,
        ),
        StorageMigrationError::Integrity | StorageMigrationError::Asset(_) => (
            "storage_integrity_failed",
            "新位置的校验没有通过，已保留原资料库。",
            false,
        ),
        StorageMigrationError::File(_)
        | StorageMigrationError::Database(_)
        | StorageMigrationError::DatabaseOpen(_)
        | StorageMigrationError::TooLarge
        | StorageMigrationError::Lock => (
            "storage_space_or_copy_failed",
            "迁移没有完成，请检查磁盘空间或连接后重试；原资料仍在原位置。",
            true,
        ),
        StorageMigrationError::MigrationPending => (
            "storage_migration_pending",
            "已有一个存储迁移等待应用重启，请先重新打开应用完成它。",
            false,
        ),
        StorageMigrationError::InvalidJournal
        | StorageMigrationError::ExpiredJournal
        | StorageMigrationError::Runtime(_)
        | StorageMigrationError::Storage(_) => (
            "storage_migration_failed",
            "存储迁移没有完成，现有资料库保持不变；请重新打开应用后再试。",
            error_is_retryable(error),
        ),
    };
    eprintln!("storage migration failed [{}]", error.code());
    AppResult::failure(code, user_message, retryable, Uuid::now_v7().to_string())
}

const fn error_is_retryable(error: &StorageMigrationError) -> bool {
    matches!(
        error,
        StorageMigrationError::File(_)
            | StorageMigrationError::Database(_)
            | StorageMigrationError::DatabaseOpen(_)
            | StorageMigrationError::Runtime(_)
            | StorageMigrationError::Lock
            | StorageMigrationError::Storage(_)
    )
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_failures_do_not_expose_internal_paths() {
        let result: AppResult<()> = storage_migration_failure(&StorageMigrationError::File(
            std::io::Error::other(r"C:\Users\secret\library.db"),
        ));
        let serialized = serde_json::to_string(&result).expect("serialize failure");

        assert!(serialized.contains("storage_space_or_copy_failed"));
        assert!(!serialized.contains(r"C:\\Users"));
        assert!(!serialized.contains("library.db"));
    }

    #[test]
    fn capture_stop_failure_prevents_a_migration_success_shape() {
        let result: AppResult<Option<StorageMigrationReceipt>> =
            capture_stop_failure(&CaptureLanError::Unavailable);
        let AppResult::Failure { error, .. } = result else {
            panic!("LAN stop failure must stop before staging")
        };

        assert_eq!(error.code, "storage_capture_stop_failed");
        assert!(error.retryable);
    }

    #[test]
    fn cancelling_the_native_folder_dialog_is_a_successful_noop() {
        let result = require_selected_destination(None).expect_err("cancel returns no destination");
        let AppResult::Success { data, .. } = result else {
            panic!("dialog cancellation must not be reported as an error")
        };

        assert_eq!(data, None);
    }
}
