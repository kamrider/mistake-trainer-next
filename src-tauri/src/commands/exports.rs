use serde::Deserialize;
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::exports::{
        CreateExportSnapshot, DeletedExportSnapshotSummary, ExportCandidate, ExportCandidateSource,
        ExportError, ExportLayout, ExportSnapshotSummary, GeneratedExportSummary,
        create_export_snapshot, delete_export_snapshot, list_deleted_export_snapshots,
        list_export_candidates, list_export_snapshots, prepare_export, restore_export_snapshot,
        write_prepared_export,
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportCreateInput {
    pub title: String,
    pub problem_ids: Vec<String>,
    pub layout: ExportLayout,
}

#[tauri::command]
#[specta::specta]
pub fn export_candidates(
    state: State<'_, LibraryRuntime>,
    source: ExportCandidateSource,
) -> AppResult<Vec<ExportCandidate>> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match list_export_candidates(
        &connection,
        state.account_id(),
        &profile.id,
        source,
        current_utc_millis(),
    ) {
        Ok(candidates) => AppResult::success(candidates),
        Err(_) => export_error("export_candidates_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_trash_list(
    state: State<'_, LibraryRuntime>,
) -> AppResult<Vec<DeletedExportSnapshotSummary>> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match list_deleted_export_snapshots(&connection, state.account_id(), &profile.id) {
        Ok(snapshots) => AppResult::success(snapshots),
        Err(_) => export_error("export_trash_list_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_list(state: State<'_, LibraryRuntime>) -> AppResult<Vec<ExportSnapshotSummary>> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match list_export_snapshots(&connection, state.account_id(), &profile.id) {
        Ok(snapshots) => AppResult::success(snapshots),
        Err(_) => export_error("export_list_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_create(
    state: State<'_, LibraryRuntime>,
    input: ExportCreateInput,
) -> AppResult<ExportSnapshotSummary> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: state.account_id().to_owned(),
            profile_id: profile.id,
            title: input.title,
            problem_ids: input.problem_ids,
            layout: input.layout,
            now_utc_ms: current_utc_millis(),
        },
    ) {
        Ok(snapshot) => AppResult::success(snapshot),
        Err(_) => export_error("export_create_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_generate(
    state: State<'_, LibraryRuntime>,
    snapshot_id: String,
) -> AppResult<Option<GeneratedExportSummary>> {
    let profile = state.active_profile();
    let prepared = {
        let connection = match state.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return export_error("library_lock_poisoned"),
        };
        match prepare_export(
            &connection,
            &state.blob_root,
            &state.asset_key,
            state.account_id(),
            &profile.id,
            &snapshot_id,
        ) {
            Ok(prepared) => prepared,
            Err(error) => return export_generation_error(error),
        }
    };
    let Some(destination) = rfd::FileDialog::new()
        .set_title("选择导出文件夹")
        .pick_folder()
    else {
        return AppResult::success(None);
    };
    match write_prepared_export(prepared, &destination) {
        Ok(summary) => AppResult::success(Some(summary)),
        Err(error) => export_generation_error(error),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_delete(state: State<'_, LibraryRuntime>, snapshot_id: String) -> AppResult<bool> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match delete_export_snapshot(
        &mut connection,
        state.account_id(),
        &profile.id,
        &snapshot_id,
        current_utc_millis(),
    ) {
        Ok(()) => AppResult::success(true),
        Err(_) => export_error("export_delete_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn export_restore(state: State<'_, LibraryRuntime>, snapshot_id: String) -> AppResult<bool> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return export_error("library_lock_poisoned"),
    };
    match restore_export_snapshot(
        &mut connection,
        state.account_id(),
        &profile.id,
        &snapshot_id,
        current_utc_millis(),
    ) {
        Ok(()) => AppResult::success(true),
        Err(_) => export_error("export_restore_failed"),
    }
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn export_error<T>(code: &str) -> AppResult<T> {
    let user_message = match code {
        "export_generate_failed" => "文件没有生成，请检查目标目录空间与权限后重试。",
        "export_restore_failed" => "导出快照没有恢复，请稍后重试。",
        "export_delete_failed" => "导出快照没有删除，请稍后重试。",
        "export_list_failed" | "export_trash_list_failed" => "导出快照没有读取成功，请稍后重试。",
        "export_candidates_failed" => "可导出的题目没有读取成功，请稍后重试。",
        _ => "导出快照没有保存，请检查选题后重试。",
    };
    AppResult::failure(code, user_message, true, Uuid::now_v7().to_string())
}

fn export_generation_error<T>(error: ExportError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        ExportError::SnapshotNotFound | ExportError::ProblemNotFound => (
            "export_snapshot_unavailable",
            "这个导出快照已删除、题目不可用，或不属于当前学习档案。",
            false,
        ),
        ExportError::InvalidImage | ExportError::InvalidAssetPath => (
            "export_asset_invalid",
            "快照中有无法读取的图片，请回到题库检查原题后重试。",
            false,
        ),
        ExportError::AssetTooLarge | ExportError::ExportTooLarge => (
            "export_too_large",
            "这份材料超过单次导出的安全上限，请减少题目数量后重新保存快照。",
            false,
        ),
        _ => (
            "export_generate_failed",
            "文件没有生成，请检查目标目录空间与权限后重试。",
            true,
        ),
    };
    AppResult::failure(code, user_message, retryable, Uuid::now_v7().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(error: ExportError) -> (String, String, bool) {
        match export_generation_error::<()>(error) {
            AppResult::Failure { error, .. } => (error.code, error.user_message, error.retryable),
            AppResult::Success { .. } => panic!("expected export failure"),
        }
    }

    #[test]
    fn generation_errors_are_stable_specific_and_path_free() {
        let missing = failure(ExportError::SnapshotNotFound);
        assert_eq!(missing.0, "export_snapshot_unavailable");
        assert!(!missing.2);

        let invalid = failure(ExportError::InvalidImage);
        assert_eq!(invalid.0, "export_asset_invalid");
        assert!(!invalid.2);

        let io = failure(ExportError::File(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            r"C:\private\student\answer.png",
        )));
        assert_eq!(io.0, "export_generate_failed");
        assert!(io.2);
        assert!(!io.1.contains("C:\\"));
    }
}
