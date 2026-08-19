use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::{assets::KeyedAssetDecryptor, runtime::LibraryRuntime},
    modules::{
        problem_bulk_metadata::{
            ProblemBulkMetadata, ProblemBulkMetadataReport, update_problem_bulk_metadata,
        },
        problems::{
            ChangeProblemStatus, ProblemDetail, ProblemDetailQuery, ProblemFilterOptions,
            ProblemFilterOptionsQuery, ProblemListInput, ProblemListPage, ProblemListQuery,
            ProblemStatusFilter, UpdateProblem, change_problem_status, get_problem_detail,
            list_problem_filter_options, list_problem_summaries_with_previews, update_problem,
        },
    },
};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryContext {
    profile_id: String,
    profile_name: String,
    storage: &'static str,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemUpdateInput {
    problem_id: String,
    subject: String,
    note: String,
    tags: Vec<String>,
    time_limit_seconds: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemStatusInput {
    problem_ids: Vec<String>,
    target_status: ProblemStatusFilter,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemBulkMetadataInput {
    pub problem_ids: Vec<String>,
    pub subject: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
}

pub fn library_context_for(runtime: &LibraryRuntime) -> AppResult<LibraryContext> {
    let profile = runtime.active_profile();
    AppResult::success(LibraryContext {
        profile_id: profile.id,
        profile_name: profile.name,
        storage: "ready",
    })
}

pub fn problem_list_for(
    runtime: &LibraryRuntime,
    input: ProblemListInput,
    now_utc_ms: i64,
) -> AppResult<ProblemListPage> {
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match list_problem_summaries_with_previews(
        &connection,
        &runtime.blob_root,
        &KeyedAssetDecryptor::new(&runtime.asset_key),
        ProblemListQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            now_utc_ms,
            input,
        },
    ) {
        Ok(problems) => AppResult::success(problems),
        Err(crate::modules::problems::ProblemUseCaseError::InvalidQuery) => AppResult::failure(
            "problem_filter_invalid",
            "筛选条件过多或过长，请精简后再试。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(_) => internal_library_error("problem_list_failed"),
    }
}

pub fn problem_filter_options_for(
    runtime: &LibraryRuntime,
    status: ProblemStatusFilter,
) -> AppResult<ProblemFilterOptions> {
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match list_problem_filter_options(
        &connection,
        ProblemFilterOptionsQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            status,
        },
    ) {
        Ok(options) => AppResult::success(options),
        Err(_) => internal_library_error("problem_filter_options_failed"),
    }
}

pub fn problem_detail_for(
    runtime: &LibraryRuntime,
    problem_id: String,
) -> AppResult<ProblemDetail> {
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match get_problem_detail(
        &connection,
        &runtime.blob_root,
        &KeyedAssetDecryptor::new(&runtime.asset_key),
        ProblemDetailQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id,
        },
    ) {
        Ok(detail) => AppResult::success(detail),
        Err(_) => internal_library_error("problem_detail_failed"),
    }
}

pub fn problem_update_for(
    runtime: &LibraryRuntime,
    input: ProblemUpdateInput,
    now_utc_ms: i64,
) -> AppResult<bool> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match update_problem(
        &mut connection,
        UpdateProblem {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id: input.problem_id,
            subject: input.subject,
            note: input.note,
            tags: input.tags,
            time_limit_seconds: input.time_limit_seconds,
            now_utc_ms,
        },
    ) {
        Ok(()) => AppResult::success(true),
        Err(crate::modules::problems::ProblemUseCaseError::InvalidTimeLimit) => AppResult::failure(
            "problem_time_limit_invalid",
            "答题时限需要填写 1 到 86400 秒，留空表示不限时。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(crate::modules::problems::ProblemUseCaseError::InvalidTags) => AppResult::failure(
            "problem_tags_invalid",
            "标签最多 20 个，每个标签最多 30 个字。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(crate::modules::problems::ProblemUseCaseError::ConflictPending) => conflict_pending(),
        Err(_) => internal_library_error("problem_update_failed"),
    }
}

pub fn problem_change_status_for(
    runtime: &LibraryRuntime,
    input: ProblemStatusInput,
    now_utc_ms: i64,
) -> AppResult<i32> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_ids: input.problem_ids,
            target_status: input.target_status,
            now_utc_ms,
        },
    ) {
        Ok(count) => AppResult::success(i32::try_from(count).unwrap_or(i32::MAX)),
        Err(crate::modules::problems::ProblemUseCaseError::ConflictPending) => conflict_pending(),
        Err(_) => internal_library_error("problem_status_failed"),
    }
}

pub fn problem_bulk_metadata_for(
    runtime: &LibraryRuntime,
    input: ProblemBulkMetadataInput,
    now_utc_ms: i64,
) -> AppResult<ProblemBulkMetadataReport> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match update_problem_bulk_metadata(
        &mut connection,
        ProblemBulkMetadata {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_ids: input.problem_ids,
            subject: input.subject,
            add_tags: input.add_tags,
            remove_tags: input.remove_tags,
            now_utc_ms,
        },
    ) {
        Ok(report) => AppResult::success(report),
        Err(crate::modules::problems::ProblemUseCaseError::ConflictPending) => conflict_pending(),
        Err(
            crate::modules::problems::ProblemUseCaseError::InvalidSelection
            | crate::modules::problems::ProblemUseCaseError::EmptyChange
            | crate::modules::problems::ProblemUseCaseError::InvalidText
            | crate::modules::problems::ProblemUseCaseError::InvalidTags,
        ) => AppResult::failure(
            "problem_bulk_metadata_invalid",
            "请选择 1 到 100 道学习中的题，并至少填写一项有效修改。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(crate::modules::problems::ProblemUseCaseError::ProblemNotFound) => AppResult::failure(
            "problem_bulk_metadata_stale",
            "部分题目已不在当前学习列表，请刷新后重新选择。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(_) => internal_library_error("problem_bulk_metadata_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn library_context(state: State<'_, LibraryRuntime>) -> AppResult<LibraryContext> {
    library_context_for(&state)
}

#[tauri::command]
#[specta::specta]
pub fn problem_list(
    state: State<'_, LibraryRuntime>,
    input: ProblemListInput,
) -> AppResult<ProblemListPage> {
    problem_list_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn problem_filter_options(
    state: State<'_, LibraryRuntime>,
    status: ProblemStatusFilter,
) -> AppResult<ProblemFilterOptions> {
    problem_filter_options_for(&state, status)
}

#[tauri::command]
#[specta::specta]
pub fn problem_detail(
    state: State<'_, LibraryRuntime>,
    problem_id: String,
) -> AppResult<ProblemDetail> {
    problem_detail_for(&state, problem_id)
}

#[tauri::command]
#[specta::specta]
pub fn problem_update(
    state: State<'_, LibraryRuntime>,
    input: ProblemUpdateInput,
) -> AppResult<bool> {
    problem_update_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn problem_change_status(
    state: State<'_, LibraryRuntime>,
    input: ProblemStatusInput,
) -> AppResult<i32> {
    problem_change_status_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn problem_bulk_metadata(
    state: State<'_, LibraryRuntime>,
    input: ProblemBulkMetadataInput,
) -> AppResult<ProblemBulkMetadataReport> {
    problem_bulk_metadata_for(&state, input, current_utc_millis())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn conflict_pending<T>() -> AppResult<T> {
    AppResult::failure(
        "problem_conflict_pending",
        "这道题有尚未处理的同步冲突，请先到“设置 → 同步冲突”选择保留本机或云端版本。",
        false,
        Uuid::now_v7().to_string(),
    )
}

fn internal_library_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "本地题库暂时无法读取，请重新打开应用后再试。",
        true,
        Uuid::now_v7().to_string(),
    )
}
