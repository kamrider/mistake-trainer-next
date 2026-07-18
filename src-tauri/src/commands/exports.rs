use serde::Deserialize;
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::exports::{
        CreateExportSnapshot, DeletedExportSnapshotSummary, ExportLayout, ExportSnapshotSummary,
        GeneratedExportSummary, create_export_snapshot, delete_export_snapshot,
        list_deleted_export_snapshots, list_export_snapshots, prepare_export,
        restore_export_snapshot, write_prepared_export,
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
    let Some(destination) = rfd::FileDialog::new()
        .set_title("选择导出文件夹")
        .pick_folder()
    else {
        return AppResult::success(None);
    };
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
            Err(_) => return export_error("export_generate_failed"),
        }
    };
    match write_prepared_export(prepared, &destination) {
        Ok(summary) => AppResult::success(Some(summary)),
        Err(_) => export_error("export_generate_failed"),
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
        _ => "导出快照没有保存，请检查选题后重试。",
    };
    AppResult::failure(code, user_message, true, Uuid::now_v7().to_string())
}
