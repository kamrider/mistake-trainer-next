use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter as _, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::{
        capture::{CaptureFileReadError, read_capture_file},
        capture_inbox::{
            ApplyCaptureLayout, CaptureBatchDetail, CaptureBatchState, CaptureBatchSummary,
            CaptureCommitReport, CaptureInboxError, CaptureItemPreview, CaptureItemSummary,
            CaptureLayoutMode, CreateCaptureBatch, IngestCaptureItem, MergeCaptureCard,
            MoveCaptureItem, StageCaptureItemRole, UpdateCaptureDraft, apply_capture_layout,
            assign_capture_batch_subject, commit_ready_capture_drafts, create_capture_batch,
            delete_capture_draft, discard_capture_batch, get_capture_batch_detail,
            get_capture_item_preview, ingest_capture_item, list_capture_batches,
            merge_capture_card, move_capture_item, remove_capture_item, stage_capture_item_role,
            update_capture_batch, update_capture_draft,
        },
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatchCreateInput {
    pub subject: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatchUpdateInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub subject: String,
    pub finish_collecting: bool,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatchSubjectInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub subject: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureImportBytesInput {
    pub batch_id: String,
    pub client_upload_id: String,
    pub source_name: String,
    pub source_sequence: Option<u32>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLayoutInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub mode: CaptureLayoutMode,
    pub question_images_per_draft: u32,
    pub answer_images_per_draft: u32,
    pub split_index: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItemMoveInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub target_draft_id: Option<String>,
    pub target_role: Option<String>,
    pub target_position: u32,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItemStageRoleInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub staged_role: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCardMergeInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub target_draft_id: Option<String>,
    pub item_ids: Vec<String>,
    pub new_draft_subject: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDraftUpdateInput {
    pub batch_id: String,
    pub expected_revision: u32,
    pub draft_id: String,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureImportReport {
    pub imported_items: Vec<CaptureItemSummary>,
    pub imported_count: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureBatchChanged {
    batch_id: String,
}

pub fn capture_batch_create_for(
    runtime: &LibraryRuntime,
    input: CaptureBatchCreateInput,
    now_utc_ms: i64,
) -> AppResult<CaptureBatchSummary> {
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(create_capture_batch(
        &mut connection,
        CreateCaptureBatch {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            subject: input.subject,
            state: CaptureBatchState::Collecting,
            now_utc_ms,
        },
    ))
}

pub fn capture_batch_list_for(runtime: &LibraryRuntime) -> AppResult<Vec<CaptureBatchSummary>> {
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(list_capture_batches(
        &connection,
        runtime.account_id(),
        runtime.profile_id(),
    ))
}

pub fn capture_batch_detail_for(
    runtime: &LibraryRuntime,
    batch_id: &str,
) -> AppResult<CaptureBatchDetail> {
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(get_capture_batch_detail(
        &connection,
        runtime.account_id(),
        runtime.profile_id(),
        batch_id,
    ))
}

pub fn capture_batch_update_for(
    runtime: &LibraryRuntime,
    input: CaptureBatchUpdateInput,
    now_utc_ms: i64,
) -> AppResult<CaptureBatchSummary> {
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(update_capture_batch(
        &mut connection,
        runtime.account_id(),
        runtime.profile_id(),
        &input.batch_id,
        input.expected_revision,
        &input.subject,
        input.finish_collecting,
        now_utc_ms,
    ))
}

pub fn capture_import_bytes_for(
    runtime: &LibraryRuntime,
    input: CaptureImportBytesInput,
    now_utc_ms: i64,
) -> AppResult<CaptureItemSummary> {
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(ingest_capture_item(
        &mut connection,
        &runtime.blob_root,
        &runtime.asset_key,
        IngestCaptureItem {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            batch_id: input.batch_id,
            client_upload_id: input.client_upload_id,
            source_name: input.source_name,
            source_sequence: input.source_sequence.map(i64::from),
            bytes: input.bytes,
            now_utc_ms,
        },
    ))
}

pub fn capture_import_paths_for(
    runtime: &LibraryRuntime,
    batch_id: &str,
    paths: Vec<std::path::PathBuf>,
    now_utc_ms: i64,
) -> AppResult<CaptureImportReport> {
    let mut prepared = Vec::with_capacity(paths.len());
    for (index, path) in paths.into_iter().enumerate() {
        let bytes = match read_capture_file(&path) {
            Ok(bytes) => bytes,
            Err(CaptureFileReadError::TooLarge) => {
                return capture_error("capture_image_too_large", None);
            }
            Err(CaptureFileReadError::Unreadable) => {
                return capture_error("capture_image_unreadable", None);
            }
        };
        let source_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image")
            .to_owned();
        prepared.push((index, source_name, bytes));
    }
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let mut imported_items = Vec::with_capacity(prepared.len());
    for (index, source_name, bytes) in prepared {
        match ingest_capture_item(
            &mut connection,
            &runtime.blob_root,
            &runtime.asset_key,
            IngestCaptureItem {
                account_id: runtime.account_id().to_owned(),
                profile_id: runtime.profile_id().to_owned(),
                batch_id: batch_id.to_owned(),
                client_upload_id: Uuid::now_v7().to_string(),
                source_name,
                source_sequence: None,
                bytes,
                now_utc_ms: now_utc_ms.saturating_add(i64::try_from(index).unwrap_or(i64::MAX)),
            },
        ) {
            Ok(item) => imported_items.push(item),
            Err(error) => return capture_error(error_code(&error), Some(&error)),
        }
    }
    AppResult::success(CaptureImportReport {
        imported_count: u32::try_from(imported_items.len()).unwrap_or(u32::MAX),
        imported_items,
    })
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_create(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureBatchCreateInput,
) -> AppResult<CaptureBatchSummary> {
    let result = capture_batch_create_for(&state, input, current_utc_millis());
    emit_batch_from_summary(&app, &result);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_list(state: State<'_, LibraryRuntime>) -> AppResult<Vec<CaptureBatchSummary>> {
    capture_batch_list_for(&state)
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_detail(
    state: State<'_, LibraryRuntime>,
    batch_id: String,
) -> AppResult<CaptureBatchDetail> {
    capture_batch_detail_for(&state, &batch_id)
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_update(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureBatchUpdateInput,
) -> AppResult<CaptureBatchSummary> {
    let result = capture_batch_update_for(&state, input, current_utc_millis());
    emit_batch_from_summary(&app, &result);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_assign_subject(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureBatchSubjectInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(assign_capture_batch_subject(
        &mut connection,
        state.account_id(),
        state.profile_id(),
        &input.batch_id,
        input.expected_revision,
        &input.subject,
        current_utc_millis(),
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_batch_discard(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    batch_id: String,
) -> AppResult<bool> {
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = match discard_capture_batch(
        &mut connection,
        &state.blob_root,
        state.account_id(),
        state.profile_id(),
        &batch_id,
    ) {
        Ok(()) => AppResult::success(true),
        Err(error) => capture_error(error_code(&error), Some(&error)),
    };
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_import_select(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    batch_id: String,
) -> AppResult<CaptureImportReport> {
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .set_title("批量选择错题图片")
        .pick_files()
    else {
        return AppResult::success(CaptureImportReport {
            imported_items: Vec::new(),
            imported_count: 0,
        });
    };
    let result = capture_import_paths_for(&state, &batch_id, paths, current_utc_millis());
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_import_bytes(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureImportBytesInput,
) -> AppResult<CaptureItemSummary> {
    let batch_id = input.batch_id.clone();
    let result = capture_import_bytes_for(&state, input, current_utc_millis());
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_item_preview(
    state: State<'_, LibraryRuntime>,
    batch_id: String,
    item_id: String,
) -> AppResult<CaptureItemPreview> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    result_or_error(get_capture_item_preview(
        &connection,
        &state.blob_root,
        &state.asset_key,
        state.account_id(),
        state.profile_id(),
        &batch_id,
        &item_id,
    ))
}

#[tauri::command]
#[specta::specta]
pub fn capture_item_remove(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    batch_id: String,
    expected_revision: u32,
    item_id: String,
) -> AppResult<CaptureBatchDetail> {
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(remove_capture_item(
        &mut connection,
        &state.blob_root,
        state.account_id(),
        state.profile_id(),
        &batch_id,
        expected_revision,
        &item_id,
        current_utc_millis(),
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_layout_apply(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureLayoutInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: state.account_id().to_owned(),
            profile_id: state.profile_id().to_owned(),
            batch_id: input.batch_id,
            expected_revision: input.expected_revision,
            mode: input.mode,
            question_images_per_draft: input.question_images_per_draft,
            answer_images_per_draft: input.answer_images_per_draft,
            split_index: input.split_index,
            now_utc_ms: current_utc_millis(),
        },
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_item_move(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureItemMoveInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(move_capture_item(
        &mut connection,
        MoveCaptureItem {
            account_id: state.account_id().to_owned(),
            profile_id: state.profile_id().to_owned(),
            batch_id: input.batch_id,
            expected_revision: input.expected_revision,
            item_id: input.item_id,
            target_draft_id: input.target_draft_id,
            target_role: input.target_role,
            target_position: input.target_position,
            now_utc_ms: current_utc_millis(),
        },
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_item_stage_role(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureItemStageRoleInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(stage_capture_item_role(
        &mut connection,
        StageCaptureItemRole {
            account_id: state.account_id().to_owned(),
            profile_id: state.profile_id().to_owned(),
            batch_id: input.batch_id,
            expected_revision: input.expected_revision,
            item_id: input.item_id,
            staged_role: input.staged_role,
            now_utc_ms: current_utc_millis(),
        },
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_card_merge(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureCardMergeInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(merge_capture_card(
        &mut connection,
        MergeCaptureCard {
            account_id: state.account_id().to_owned(),
            profile_id: state.profile_id().to_owned(),
            batch_id: input.batch_id,
            expected_revision: input.expected_revision,
            target_draft_id: input.target_draft_id,
            item_ids: input.item_ids,
            new_draft_subject: input.new_draft_subject,
            now_utc_ms: current_utc_millis(),
        },
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_draft_delete(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    batch_id: String,
    expected_revision: u32,
    draft_id: String,
) -> AppResult<CaptureBatchDetail> {
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(delete_capture_draft(
        &mut connection,
        state.account_id(),
        state.profile_id(),
        &batch_id,
        &draft_id,
        expected_revision,
        current_utc_millis(),
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_draft_update(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    input: CaptureDraftUpdateInput,
) -> AppResult<CaptureBatchDetail> {
    let batch_id = input.batch_id.clone();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(update_capture_draft(
        &mut connection,
        UpdateCaptureDraft {
            account_id: state.account_id().to_owned(),
            profile_id: state.profile_id().to_owned(),
            batch_id: input.batch_id,
            expected_revision: input.expected_revision,
            draft_id: input.draft_id,
            subject: input.subject,
            tags: input.tags,
            note: input.note,
            now_utc_ms: current_utc_millis(),
        },
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

#[tauri::command]
#[specta::specta]
pub fn capture_commit_ready(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    batch_id: String,
    expected_revision: u32,
) -> AppResult<CaptureCommitReport> {
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return capture_error("library_lock_poisoned", None),
    };
    let result = result_or_error(commit_ready_capture_drafts(
        &mut connection,
        state.account_id(),
        state.profile_id(),
        &batch_id,
        expected_revision,
        current_utc_millis(),
    ));
    emit_batch_changed(&app, &batch_id);
    result
}

fn result_or_error<T>(result: Result<T, CaptureInboxError>) -> AppResult<T> {
    match result {
        Ok(value) => AppResult::success(value),
        Err(error) => capture_error(error_code(&error), Some(&error)),
    }
}

fn error_code(error: &CaptureInboxError) -> &'static str {
    match error {
        CaptureInboxError::BatchNotFound => "capture_batch_not_found",
        CaptureInboxError::DraftNotFound => "capture_draft_not_found",
        CaptureInboxError::ItemNotFound => "capture_item_not_found",
        CaptureInboxError::RevisionConflict => "capture_revision_conflict",
        CaptureInboxError::InvalidState => "capture_state_invalid",
        CaptureInboxError::InvalidInput => "capture_input_invalid",
        CaptureInboxError::CapacityReached => "capture_batch_capacity_reached",
        CaptureInboxError::InvalidImage => "capture_image_invalid",
        CaptureInboxError::InvalidAssetPath => "capture_asset_path_invalid",
        CaptureInboxError::Io(_) => "capture_file_failed",
        CaptureInboxError::Database(_) => "capture_database_failed",
        CaptureInboxError::Serialization(_) => "capture_serialization_failed",
        CaptureInboxError::Crypto => "capture_crypto_failed",
    }
}

fn capture_error<T>(code: &str, error: Option<&CaptureInboxError>) -> AppResult<T> {
    let (message, retryable) = match code {
        "capture_batch_not_found" => ("这个采集批次已经不存在，请返回采集箱刷新。", false),
        "capture_draft_not_found" => ("这道草稿已经变化，请刷新批次后重试。", false),
        "capture_item_not_found" => ("这张图片已经被移动或删除，请刷新后重试。", false),
        "capture_revision_conflict" => ("采集批次刚刚发生了变化，已阻止旧操作覆盖新内容。", true),
        "capture_state_invalid" => ("当前批次状态不允许执行这个操作。", false),
        "capture_input_invalid" => ("采集内容不完整或超过长度限制，请检查后重试。", false),
        "capture_batch_capacity_reached" => {
            ("单批最多 150 张、合计 1 GB，请先整理或新建批次。", false)
        }
        "capture_image_too_large" => ("单张图片不能超过 25 MB，请压缩后重试。", false),
        "capture_image_unreadable" => ("有图片无法读取，请确认文件仍然存在且未损坏。", false),
        "capture_image_invalid" => ("有图片损坏或格式不受支持；支持 PNG、JPEG 与 WebP。", false),
        "capture_asset_path_invalid" => ("本地资产路径校验失败，已停止读取。", false),
        "library_lock_poisoned" => ("本地题库暂时不可用，请重新打开应用后重试。", true),
        _ => (
            "采集箱没有完成这次操作，原有草稿仍会保留，请稍后重试。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!("capture inbox error [{diagnostic_id}] {code}: {error}");
    }
    AppResult::failure(code, message, retryable, diagnostic_id)
}

fn emit_batch_from_summary(app: &AppHandle, result: &AppResult<CaptureBatchSummary>) {
    if let AppResult::Success { data, .. } = result {
        emit_batch_changed(app, &data.id);
    }
}

fn emit_batch_changed(app: &AppHandle, batch_id: &str) {
    let _ = app.emit(
        "capture_batch_changed",
        CaptureBatchChanged {
            batch_id: batch_id.to_owned(),
        },
    );
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
