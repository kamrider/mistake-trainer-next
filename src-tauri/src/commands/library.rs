use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::problems::{
        ChangeProblemStatus, ProblemDetail, ProblemDetailQuery, ProblemListQuery,
        ProblemStatusFilter, ProblemSummary, UpdateProblem, change_problem_status,
        get_problem_detail, list_problem_summaries, update_problem,
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
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemStatusInput {
    problem_ids: Vec<String>,
    target_status: ProblemStatusFilter,
}

pub fn library_context_for(runtime: &LibraryRuntime) -> AppResult<LibraryContext> {
    AppResult::success(LibraryContext {
        profile_id: runtime.profile_id().to_owned(),
        profile_name: runtime.profile_name().to_owned(),
        storage: "ready",
    })
}

pub fn problem_list_for(
    runtime: &LibraryRuntime,
    status: ProblemStatusFilter,
    search: Option<String>,
) -> AppResult<Vec<ProblemSummary>> {
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            status,
            search,
        },
    ) {
        Ok(problems) => AppResult::success(problems),
        Err(_) => internal_library_error("problem_list_failed"),
    }
}

pub fn problem_detail_for(runtime: &LibraryRuntime, problem_id: String) -> AppResult<ProblemDetail> {
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match get_problem_detail(
        &connection,
        &runtime.blob_root,
        &runtime.asset_key,
        ProblemDetailQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
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
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match update_problem(
        &mut connection,
        UpdateProblem {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            problem_id: input.problem_id,
            subject: input.subject,
            note: input.note,
            now_utc_ms,
        },
    ) {
        Ok(()) => AppResult::success(true),
        Err(_) => internal_library_error("problem_update_failed"),
    }
}

pub fn problem_change_status_for(
    runtime: &LibraryRuntime,
    input: ProblemStatusInput,
    now_utc_ms: i64,
) -> AppResult<i32> {
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_library_error("library_lock_poisoned"),
    };
    match change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            problem_ids: input.problem_ids,
            target_status: input.target_status,
            now_utc_ms,
        },
    ) {
        Ok(count) => AppResult::success(i32::try_from(count).unwrap_or(i32::MAX)),
        Err(_) => internal_library_error("problem_status_failed"),
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
    status: ProblemStatusFilter,
    search: Option<String>,
) -> AppResult<Vec<ProblemSummary>> {
    problem_list_for(&state, status, search)
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

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn internal_library_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "本地题库暂时无法读取，请重新打开应用后再试。",
        true,
        Uuid::now_v7().to_string(),
    )
}
