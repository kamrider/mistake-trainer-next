use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::backup::{BackupError, BackupSummary, create_backup, validate_backup},
};

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
pub async fn backup_validate(
    state: State<'_, LibraryRuntime>,
) -> Result<AppResult<Option<BackupSummary>>, ()> {
    let database_key = state.database_key().to_owned();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let source = rfd::FileDialog::new()
            .set_title("选择要验证的 Mistake Trainer 备份目录")
            .pick_folder();
        validate_selected_package(&database_key, &asset_key, &account_id, source)
    });
    Ok(match worker.await {
        Ok(Ok(summary)) => AppResult::success(summary),
        Ok(Err(error)) => backup_failure("backup_validate_failed", &error),
        Err(_) => backup_failure("backup_validate_failed", &BackupError::Lock),
    })
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

fn validate_selected_package(
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    source: Option<PathBuf>,
) -> Result<Option<BackupSummary>, BackupError> {
    let Some(source) = source else {
        return Ok(None);
    };
    validate_backup(&source, database_key, asset_key, account_id).map(Some)
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
    use super::{backup_failure, validate_selected_package};
    use crate::modules::backup::BackupError;

    #[test]
    fn cancelling_package_selection_returns_no_summary() {
        let result = validate_selected_package("unused", &[0_u8; 32], "unused", None)
            .expect("cancel succeeds");
        assert_eq!(result, None);
    }

    #[test]
    fn public_failures_never_serialize_internal_paths() {
        let result: crate::application::result::AppResult<()> = backup_failure(
            "backup_validate_failed",
            &BackupError::Io(std::io::Error::other("C:\\Users\\private\\backup")),
        );
        let serialized = serde_json::to_string(&result).expect("serialize AppResult");
        assert!(!serialized.contains("Users"));
        assert!(!serialized.contains("private"));
        assert!(serialized.contains("现有资料库未被替换"));
    }
}
