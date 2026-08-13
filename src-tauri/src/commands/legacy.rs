use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::legacy::{
        LegacyImportCandidate, LegacyImportError, LegacyImportManager, LegacyImportReceipt,
        LegacyImportSummary, LegacyRollbackReceipt, import_legacy_plan, list_legacy_imports,
        rollback_legacy_import,
    },
};

#[tauri::command]
#[specta::specta]
pub async fn legacy_scan(
    manager: State<'_, LegacyImportManager>,
) -> Result<AppResult<Option<LegacyImportCandidate>>, ()> {
    let manager = manager.inner().clone();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let root = rfd::FileDialog::new()
            .set_title("选择旧版错题软件数据目录")
            .pick_folder();
        prepare_selected_root(&manager, root, current_utc_millis())
    });
    Ok(match worker.await {
        Ok(Ok(candidate)) => AppResult::success(candidate),
        Ok(Err(error)) => legacy_failure("legacy_scan_failed", &error),
        Err(_) => legacy_failure("legacy_scan_failed", &LegacyImportError::UnsafeSource),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_import(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    manager: State<'_, LegacyImportManager>,
    candidate_id: String,
) -> Result<AppResult<LegacyImportReceipt>, ()> {
    let now = current_utc_millis();
    let plan = match manager.plan_for(&candidate_id, now) {
        Ok(plan) => plan,
        Err(error) => return Ok(legacy_failure("legacy_candidate_missing", &error)),
    };
    let connection = state.connection.clone();
    let transition = state.profile_transition_lock();
    let blob_root = state.blob_root.clone();
    let key = state.asset_key;
    let account_id = state.account_id().to_owned();
    let manager = manager.inner().clone();
    drop(state);
    let completed_candidate_id = candidate_id.clone();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let _transition = transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut database = connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let asset_encryptor = crate::infrastructure::assets::KeyedAssetEncryptor::new(&key);
        let result = import_legacy_plan(
            &mut database,
            &blob_root,
            &asset_encryptor,
            &account_id,
            &candidate_id,
            plan,
            now,
            |progress| {
                let _ = app.emit("legacy_import_progress", progress);
            },
        );
        if result.is_ok() {
            manager.consume(&completed_candidate_id);
        }
        result
    });
    Ok(match worker.await {
        Ok(Ok(receipt)) => AppResult::success(receipt),
        Ok(Err(error)) => legacy_failure("legacy_import_failed", &error),
        Err(_) => legacy_failure("legacy_import_failed", &LegacyImportError::UnsafeSource),
    })
}

#[tauri::command]
#[specta::specta]
pub fn legacy_import_list(state: State<'_, LibraryRuntime>) -> AppResult<Vec<LegacyImportSummary>> {
    let database = state
        .connection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match list_legacy_imports(&database, state.account_id()) {
        Ok(imports) => AppResult::success(imports),
        Err(error) => legacy_failure("legacy_import_list_failed", &error),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_rollback(
    state: State<'_, LibraryRuntime>,
    import_id: String,
) -> Result<AppResult<LegacyRollbackReceipt>, ()> {
    let connection = state.connection.clone();
    let transition = state.profile_transition_lock();
    let blob_root = state.blob_root.clone();
    let account_id = state.account_id().to_owned();
    drop(state);
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let _transition = transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut database = connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rollback_legacy_import(
            &mut database,
            &blob_root,
            &account_id,
            &import_id,
            current_utc_millis(),
        )
    });
    Ok(match worker.await {
        Ok(Ok(receipt)) => AppResult::success(receipt),
        Ok(Err(error)) => legacy_failure("legacy_rollback_failed", &error),
        Err(_) => legacy_failure("legacy_rollback_failed", &LegacyImportError::UnsafeSource),
    })
}

fn prepare_selected_root(
    manager: &LegacyImportManager,
    root: Option<PathBuf>,
    now_utc_ms: i64,
) -> Result<Option<LegacyImportCandidate>, LegacyImportError> {
    root.map(|path| manager.prepare(&path, now_utc_ms))
        .transpose()
}

fn legacy_failure<T>(code: &str, error: &LegacyImportError) -> AppResult<T> {
    let (message, retryable) = match error {
        LegacyImportError::ImportNotFound => {
            ("这次迁移预检已过期或不存在，请重新选择旧版目录。", false)
        }
        LegacyImportError::AlreadyImported => ("这份旧版资料已经成功导入过，无需重复导入。", false),
        LegacyImportError::InvalidImage => {
            ("旧版资料中有无法安全解码的图片，尚未写入任何数据。", false)
        }
        LegacyImportError::SourceChanged => (
            "旧版目录在迁移期间发生了变化，已撤销本次写入，请重新预检。",
            true,
        ),
        _ => (
            "迁移没有完成；旧版目录未被修改，新资料库已回滚，请检查磁盘空间后重试。",
            true,
        ),
    };
    AppResult::failure(code, message, retryable, Uuid::now_v7().to_string())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{legacy_failure, prepare_selected_root};
    use crate::modules::legacy::{LegacyImportError, LegacyImportManager};

    #[test]
    fn cancelling_the_folder_picker_returns_no_candidate() {
        assert_eq!(
            prepare_selected_root(&LegacyImportManager::default(), None, 1)
                .expect("cancel succeeds"),
            None
        );
    }

    #[test]
    fn public_failures_never_serialize_internal_paths() {
        let result: crate::application::result::AppResult<()> = legacy_failure(
            "legacy_import_failed",
            &LegacyImportError::Io(std::io::Error::other("C:\\Users\\private\\question.png")),
        );
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("private"));
        assert!(!serialized.contains("question.png"));
    }
}
