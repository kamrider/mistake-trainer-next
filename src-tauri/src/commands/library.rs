use serde::Serialize;
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::problems::{
        ProblemListQuery, ProblemStatusFilter, ProblemSummary, list_problem_summaries,
    },
};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryContext {
    profile_id: String,
    profile_name: String,
    storage: &'static str,
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
        },
    ) {
        Ok(problems) => AppResult::success(problems),
        Err(_) => internal_library_error("problem_list_failed"),
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
) -> AppResult<Vec<ProblemSummary>> {
    problem_list_for(&state, status)
}

fn internal_library_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "本地题库暂时无法读取，请重新打开应用后再试。",
        true,
        Uuid::now_v7().to_string(),
    )
}
