use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    domain::review::FsrsRating,
    infrastructure::runtime::LibraryRuntime,
    modules::{
        problems::{ProblemDetail, ProblemDetailQuery, get_problem_detail},
        review::{
            BeginExamGrading, NavigateExam, QuickReviewPreset, ReviewQueueQuery, ReviewQueueState,
            ReviewSubmission, StartExamReview, StartManualReview, StartQuickReview, SubmitReview,
            begin_exam_grading, list_review_queue, navigate_exam, start_exam_review_queue,
            start_manual_review_queue, start_quick_review_queue, submit_review,
        },
        review_focus::{
            FocusNumberSelection, ReviewFocusError, ReviewFocusState, SkipReviewFocus,
            select_focus_number, skip_focus_round,
        },
    },
};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueItem {
    pub problem_id: String,
    pub due_at_utc_ms: Option<f64>,
    pub review_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueOverview {
    pub session_id: Option<String>,
    pub mode: String,
    pub resumed: bool,
    pub completed_count: i32,
    pub total_count: i32,
    pub exam_phase: Option<String>,
    pub exam_question_index: i32,
    pub exam_correct_count: i32,
    pub exam_wrong_count: i32,
    pub focus: Option<ReviewFocusState>,
    pub items: Vec<ReviewQueueItem>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSubmitInput {
    pub problem_id: String,
    pub rating: FsrsRating,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewManualStartInput {
    pub problem_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQuickStartInput {
    pub preset: QuickReviewPreset,
    pub subject: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewExamStartInput {
    pub problem_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewExamNavigateInput {
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFocusSelectInput {
    pub number: i32,
    pub elapsed_ms: u32,
}

pub fn review_queue_for(
    runtime: &LibraryRuntime,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    let overview = match list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            now_utc_ms,
        },
    ) {
        Ok(overview) => overview,
        Err(_) => return internal_review_error("review_queue_failed"),
    };
    AppResult::success(queue_overview(overview))
}

pub fn review_current_problem_for(runtime: &LibraryRuntime) -> AppResult<ProblemDetail> {
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    let selected = connection
        .query_row(
            "SELECT json_extract(
                        problem_ids_json,
                        '$[' || CASE
                            WHEN experience = 'exam' AND exam_phase = 'answering'
                            THEN exam_question_index
                            ELSE current_index
                        END || ']'
                    ),
                    CASE WHEN experience = 'exam' AND exam_phase = 'answering' THEN 1 ELSE 0 END
             FROM review_sessions
             WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'
               AND focus_order_json IS NULL",
            params![runtime.account_id(), profile.id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional();
    let (problem_id, hide_answer) = match selected {
        Ok(Some(selected)) => selected,
        Ok(None) => return invalid_review_state("review_current_problem_missing"),
        Err(_) => return internal_review_error("review_current_problem_lookup_failed"),
    };
    match get_problem_detail(
        &connection,
        &runtime.blob_root,
        &runtime.asset_key,
        ProblemDetailQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id,
        },
    ) {
        Ok(mut detail) => {
            if hide_answer {
                detail.assets.retain(|asset| asset.role == "question");
            }
            AppResult::success(detail)
        }
        Err(_) => internal_review_error("review_current_problem_failed"),
    }
}

pub fn review_manual_start_for(
    runtime: &LibraryRuntime,
    input: ReviewManualStartInput,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match start_manual_review_queue(
        &mut connection,
        StartManualReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_ids: input.problem_ids,
            now_utc_ms,
        },
    ) {
        Ok(overview) => AppResult::success(queue_overview(overview)),
        Err(crate::modules::review::ReviewUseCaseError::InvalidManualSelection) => {
            AppResult::failure(
                "review_manual_selection_invalid",
                "所选题目已经变化，请回到题库重新选择后再试。",
                false,
                Uuid::now_v7().to_string(),
            )
        }
        Err(_) => internal_review_error("review_manual_start_failed"),
    }
}

pub fn review_quick_start_for(
    runtime: &LibraryRuntime,
    input: ReviewQuickStartInput,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match start_quick_review_queue(
        &mut connection,
        StartQuickReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            preset: input.preset,
            subject: input.subject,
            tag: input.tag,
            now_utc_ms,
        },
    ) {
        Ok(overview) => AppResult::success(queue_overview(overview)),
        Err(crate::modules::review::ReviewUseCaseError::NoQuickCandidates) => AppResult::failure(
            "review_quick_empty",
            "当前没有符合条件的题目，可以调整科目或标签后再试。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(crate::modules::review::ReviewUseCaseError::InvalidQuickSelection) => {
            AppResult::failure(
                "review_quick_filter_invalid",
                "快速训练的科目或标签过长，请精简后再试。",
                false,
                Uuid::now_v7().to_string(),
            )
        }
        Err(_) => internal_review_error("review_quick_start_failed"),
    }
}

pub fn review_exam_start_for(
    runtime: &LibraryRuntime,
    input: ReviewExamStartInput,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match start_exam_review_queue(
        &mut connection,
        StartExamReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_ids: input.problem_ids,
            now_utc_ms,
        },
    ) {
        Ok(overview) => AppResult::success(queue_overview(overview)),
        Err(crate::modules::review::ReviewUseCaseError::InvalidExamSelection) => {
            AppResult::failure(
                "review_exam_selection_invalid",
                "所选题目已经变化，请回到题库重新选择后再试。",
                false,
                Uuid::now_v7().to_string(),
            )
        }
        Err(_) => internal_review_error("review_exam_start_failed"),
    }
}

pub fn review_exam_navigate_for(
    runtime: &LibraryRuntime,
    input: ReviewExamNavigateInput,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match navigate_exam(
        &mut connection,
        NavigateExam {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            position: input.position,
            now_utc_ms,
        },
    ) {
        Ok(overview) => AppResult::success(queue_overview(overview)),
        Err(crate::modules::review::ReviewUseCaseError::InvalidExamState) => invalid_exam_state(),
        Err(_) => internal_review_error("review_exam_navigate_failed"),
    }
}

pub fn review_exam_begin_grading_for(
    runtime: &LibraryRuntime,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match begin_exam_grading(
        &mut connection,
        BeginExamGrading {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            now_utc_ms,
        },
    ) {
        Ok(overview) => AppResult::success(queue_overview(overview)),
        Err(crate::modules::review::ReviewUseCaseError::InvalidExamState) => invalid_exam_state(),
        Err(_) => internal_review_error("review_exam_begin_grading_failed"),
    }
}

fn queue_overview(overview: ReviewQueueState) -> ReviewQueueOverview {
    let items = overview
        .items
        .into_iter()
        .map(|entry| ReviewQueueItem {
            problem_id: entry.problem_id,
            due_at_utc_ms: entry.due_at_utc_ms,
            review_count: entry.review_count,
        })
        .collect();
    ReviewQueueOverview {
        session_id: overview.session_id,
        mode: overview.mode,
        resumed: overview.resumed,
        completed_count: overview.completed_count,
        total_count: overview.total_count,
        exam_phase: overview.exam_phase,
        exam_question_index: overview.exam_question_index,
        exam_correct_count: overview.exam_correct_count,
        exam_wrong_count: overview.exam_wrong_count,
        focus: overview.focus,
        items,
    }
}

pub fn review_submit_for(
    runtime: &LibraryRuntime,
    input: ReviewSubmitInput,
    now_utc_ms: i64,
) -> AppResult<ReviewSubmission> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match submit_review(
        &mut connection,
        SubmitReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id: input.problem_id,
            device_id: runtime.device_id().to_owned(),
            rating: input.rating,
            duration_ms: input.duration_ms.min(86_400_000),
            occurred_at_utc_ms: now_utc_ms,
        },
    ) {
        Ok(submission) => AppResult::success(submission),
        Err(_) => internal_review_error("review_submit_failed"),
    }
}

pub fn review_focus_select_for(
    runtime: &LibraryRuntime,
    input: ReviewFocusSelectInput,
    now_utc_ms: i64,
) -> AppResult<Option<ReviewFocusState>> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match select_focus_number(
        &mut connection,
        FocusNumberSelection {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            number: input.number,
            elapsed_ms: input.elapsed_ms,
            now_utc_ms,
        },
    ) {
        Ok(focus) => AppResult::success(focus),
        Err(ReviewFocusError::StateChanged) => invalid_focus_state(),
        Err(_) => internal_review_error("review_focus_select_failed"),
    }
}

pub fn review_focus_skip_for(
    runtime: &LibraryRuntime,
    now_utc_ms: i64,
) -> AppResult<Option<ReviewFocusState>> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match skip_focus_round(
        &mut connection,
        SkipReviewFocus {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            now_utc_ms,
        },
    ) {
        Ok(focus) => AppResult::success(focus),
        Err(ReviewFocusError::StateChanged) => invalid_focus_state(),
        Err(_) => internal_review_error("review_focus_skip_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn review_queue(state: State<'_, LibraryRuntime>) -> AppResult<ReviewQueueOverview> {
    review_queue_for(&state, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_current_problem(state: State<'_, LibraryRuntime>) -> AppResult<ProblemDetail> {
    review_current_problem_for(&state)
}

#[tauri::command]
#[specta::specta]
pub fn review_manual_start(
    state: State<'_, LibraryRuntime>,
    input: ReviewManualStartInput,
) -> AppResult<ReviewQueueOverview> {
    review_manual_start_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_quick_start(
    state: State<'_, LibraryRuntime>,
    input: ReviewQuickStartInput,
) -> AppResult<ReviewQueueOverview> {
    review_quick_start_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_exam_start(
    state: State<'_, LibraryRuntime>,
    input: ReviewExamStartInput,
) -> AppResult<ReviewQueueOverview> {
    review_exam_start_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_exam_navigate(
    state: State<'_, LibraryRuntime>,
    input: ReviewExamNavigateInput,
) -> AppResult<ReviewQueueOverview> {
    review_exam_navigate_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_exam_begin_grading(
    state: State<'_, LibraryRuntime>,
) -> AppResult<ReviewQueueOverview> {
    review_exam_begin_grading_for(&state, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_submit(
    state: State<'_, LibraryRuntime>,
    input: ReviewSubmitInput,
) -> AppResult<ReviewSubmission> {
    review_submit_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_focus_select(
    state: State<'_, LibraryRuntime>,
    input: ReviewFocusSelectInput,
) -> AppResult<Option<ReviewFocusState>> {
    review_focus_select_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_focus_skip(state: State<'_, LibraryRuntime>) -> AppResult<Option<ReviewFocusState>> {
    review_focus_skip_for(&state, current_utc_millis())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn internal_review_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "训练记录暂时无法读取或保存，请稍后重试。",
        true,
        Uuid::now_v7().to_string(),
    )
}

fn invalid_exam_state<T>() -> AppResult<T> {
    AppResult::failure(
        "review_exam_state_changed",
        "考试进度已经变化，请重新打开训练室继续。",
        true,
        Uuid::now_v7().to_string(),
    )
}

fn invalid_review_state<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "训练进度已经变化，请重新打开训练室继续。",
        true,
        Uuid::now_v7().to_string(),
    )
}

fn invalid_focus_state<T>() -> AppResult<T> {
    AppResult::failure(
        "review_focus_state_changed",
        "专注进度已经变化，请重新打开训练室继续。",
        true,
        Uuid::now_v7().to_string(),
    )
}
