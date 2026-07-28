use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    commands::storage::ApplicationControlRoot,
    infrastructure::{
        capture_recognition_worker::{
            CaptureRecognitionEventSink, CaptureRecognitionManager, CaptureRecognitionWorkerContext,
        },
        runtime::LibraryRuntime,
    },
    modules::{
        capture_recognition::{
            ApplyCaptureRecognition, CaptureRecognitionApplyReport, CaptureRecognitionDecision,
            CaptureRecognitionError, CaptureRecognitionJob, CaptureRecognitionOperationSummary,
            CaptureRecognitionRegionProposal, CaptureRecognitionRevertReport,
            CreateCaptureRecognitionJob, RevertCaptureRecognition,
            ReviewCaptureRecognitionSuggestion, apply_capture_recognition, cancel_recognition_job,
            create_or_resume_recognition_job, get_active_recognition_job,
            latest_capture_recognition_operation, revert_capture_recognition,
            review_recognition_suggestion,
        },
        ocr_capability::{OcrRecognitionFeatureState, capability_status},
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionStartInput {
    pub batch_id: String,
    pub item_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionReviewInput {
    pub job_id: String,
    pub suggestion_id: String,
    pub decision: CaptureRecognitionDecision,
    pub edited_regions: Option<Vec<CaptureRecognitionRegionProposal>>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionApplyInput {
    pub batch_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub accepted_suggestion_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionRevertInput {
    pub batch_id: String,
    pub operation_id: String,
    pub expected_revision: u32,
}

#[tauri::command]
#[specta::specta]
pub fn capture_recognition_start(
    app: AppHandle,
    input: CaptureRecognitionStartInput,
    control_root: State<'_, ApplicationControlRoot>,
    runtime: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureRecognitionManager>,
) -> AppResult<CaptureRecognitionJob> {
    let capability = match capability_status(&control_root.0) {
        Ok(capability) => capability,
        Err(_) => return recognition_failure("capture_recognition_failed", None),
    };
    match capability.recognition_feature.state {
        OcrRecognitionFeatureState::EvidenceGatePending => {
            recognition_failure("capture_recognition_gate_closed", None)
        }
        OcrRecognitionFeatureState::RuntimeMissing => {
            recognition_failure("capture_recognition_runtime_missing", None)
        }
        OcrRecognitionFeatureState::ModelMissing => {
            recognition_failure("capture_recognition_model_missing", None)
        }
        OcrRecognitionFeatureState::Ready => {
            if !manager.product_engine_configured() {
                return recognition_failure("capture_recognition_runtime_missing", None);
            }
            let profile = runtime.active_profile();
            let account_id = runtime.account_id().to_owned();
            let job = {
                let mut connection = match runtime.connection.lock() {
                    Ok(connection) => connection,
                    Err(_) => return recognition_failure("library_lock_poisoned", None),
                };
                match create_or_resume_recognition_job(
                    &mut connection,
                    CreateCaptureRecognitionJob {
                        account_id: account_id.clone(),
                        profile_id: profile.id.clone(),
                        batch_id: input.batch_id.clone(),
                        item_ids: input.item_ids,
                        engine: "local-visual-whitespace".to_owned(),
                        engine_version: "1.0.0".to_owned(),
                        now_utc_ms: current_utc_millis(),
                    },
                ) {
                    Ok(job) => job,
                    Err(error) => return recognition_failure(error.code(), Some(&error)),
                }
            };
            let context = CaptureRecognitionWorkerContext {
                connection: runtime.connection.clone(),
                account_id,
                profile_id: profile.id,
                batch_id: job.batch_id.clone(),
                job_id: job.id.clone(),
                blob_root: runtime.blob_root.clone(),
                private_temp_root: control_root.0.join("recognition-private-temp"),
                asset_key: runtime.asset_key,
            };
            let worker = manager.inner().clone();
            let event_app = app.clone();
            let emit: CaptureRecognitionEventSink = std::sync::Arc::new(move |event| {
                let _ = event_app.emit("capture_recognition_changed", event);
            });
            tauri::async_runtime::spawn(async move {
                if worker.run_job(context, emit).await.is_err() {
                    eprintln!("capture recognition worker stopped unexpectedly");
                }
            });
            AppResult::success(job)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn capture_recognition_status(
    state: State<'_, LibraryRuntime>,
    batch_id: String,
) -> AppResult<Option<CaptureRecognitionJob>> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return recognition_failure("library_lock_poisoned", None),
    };
    match get_active_recognition_job(&connection, state.account_id(), &profile.id, &batch_id) {
        Ok(job) => AppResult::success(job),
        Err(error) => recognition_failure(error.code(), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn capture_recognition_review(
    state: State<'_, LibraryRuntime>,
    input: CaptureRecognitionReviewInput,
) -> AppResult<CaptureRecognitionJob> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return recognition_failure("library_lock_poisoned", None),
    };
    match review_recognition_suggestion(
        &mut connection,
        ReviewCaptureRecognitionSuggestion {
            account_id: state.account_id().to_owned(),
            profile_id: profile.id,
            job_id: input.job_id,
            suggestion_id: input.suggestion_id,
            decision: input.decision,
            edited_regions: input.edited_regions,
            now_utc_ms: current_utc_millis(),
        },
    ) {
        Ok(job) => AppResult::success(job),
        Err(error) => recognition_failure(error.code(), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn capture_recognition_cancel(
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureRecognitionManager>,
    job_id: String,
) -> Result<AppResult<CaptureRecognitionJob>, ()> {
    let _guard = manager.lock_mutation().await;
    manager.cancel(&job_id).await;
    let profile = state.active_profile();
    let result = {
        let mut connection = match state.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return Ok(recognition_failure("library_lock_poisoned", None)),
        };
        match cancel_recognition_job(
            &mut connection,
            state.account_id(),
            &profile.id,
            &job_id,
            current_utc_millis(),
        ) {
            Ok(job) => AppResult::success(job),
            Err(error) => recognition_failure(error.code(), Some(&error)),
        }
    };
    manager.clear(&job_id).await;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn capture_recognition_apply(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureRecognitionManager>,
    input: CaptureRecognitionApplyInput,
) -> Result<AppResult<CaptureRecognitionApplyReport>, ()> {
    let _guard = manager.lock_mutation().await;
    let batch_id = input.batch_id.clone();
    let profile = state.active_profile();
    let result = {
        let mut connection = match state.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return Ok(recognition_failure("library_lock_poisoned", None)),
        };
        match apply_capture_recognition(
            &mut connection,
            ApplyCaptureRecognition {
                account_id: state.account_id().to_owned(),
                profile_id: profile.id,
                batch_id: input.batch_id,
                job_id: input.job_id,
                expected_revision: input.expected_revision,
                accepted_suggestion_ids: input.accepted_suggestion_ids,
                blob_root: state.blob_root.clone(),
                asset_key: state.asset_key,
                now_utc_ms: current_utc_millis(),
                failure_point: None,
            },
        ) {
            Ok(report) => AppResult::success(report),
            Err(error) => recognition_failure(error.code(), Some(&error)),
        }
    };
    if matches!(result, AppResult::Success { .. }) {
        let _ = app.emit(
            "capture_batch_changed",
            serde_json::json!({ "batchId": batch_id }),
        );
    }
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn capture_recognition_revert(
    app: AppHandle,
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureRecognitionManager>,
    input: CaptureRecognitionRevertInput,
) -> Result<AppResult<CaptureRecognitionRevertReport>, ()> {
    let _guard = manager.lock_mutation().await;
    let batch_id = input.batch_id.clone();
    let profile = state.active_profile();
    let result = {
        let mut connection = match state.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return Ok(recognition_failure("library_lock_poisoned", None)),
        };
        match revert_capture_recognition(
            &mut connection,
            RevertCaptureRecognition {
                account_id: state.account_id().to_owned(),
                profile_id: profile.id,
                batch_id: input.batch_id,
                operation_id: input.operation_id,
                expected_revision: input.expected_revision,
                blob_root: state.blob_root.clone(),
                now_utc_ms: current_utc_millis(),
            },
        ) {
            Ok(report) => AppResult::success(report),
            Err(error) => recognition_failure(error.code(), Some(&error)),
        }
    };
    if matches!(result, AppResult::Success { .. }) {
        let _ = app.emit(
            "capture_batch_changed",
            serde_json::json!({ "batchId": batch_id }),
        );
    }
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn capture_recognition_last_operation(
    state: State<'_, LibraryRuntime>,
    batch_id: String,
) -> AppResult<Option<CaptureRecognitionOperationSummary>> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return recognition_failure("library_lock_poisoned", None),
    };
    match latest_capture_recognition_operation(
        &connection,
        state.account_id(),
        &profile.id,
        &batch_id,
    ) {
        Ok(operation) => AppResult::success(operation),
        Err(error) => recognition_failure(error.code(), Some(&error)),
    }
}

fn recognition_failure<T>(code: &str, error: Option<&CaptureRecognitionError>) -> AppResult<T> {
    let (stable_code, user_message, retryable) = match code {
        "capture_recognition_model_missing" => (
            code,
            "需要先下载并校验本地识别模型；当前草稿没有变化。",
            false,
        ),
        "capture_recognition_gate_closed" => (
            code,
            "智能分题仍在真实题图验证中；请继续使用顺序模板或手工整理。",
            false,
        ),
        "capture_recognition_stale" => (
            code,
            "部分图片已被移动或修改；这些建议已退出本次应用。",
            true,
        ),
        "capture_recognition_busy" => (code, "这个批次已有识别任务，已为你恢复现有进度。", true),
        "capture_recognition_revision_conflict" => (
            code,
            "采集批次刚刚发生了变化，旧的识别结果没有覆盖新内容。请刷新后重试。",
            true,
        ),
        "capture_recognition_capacity_reached" => (
            code,
            "应用后的图片会超过本批次 150 张或 1 GB 上限；请减少选择后重试。",
            false,
        ),
        "capture_recognition_revert_conflict" => (
            code,
            "应用后的题卡已经被继续编辑或入库，为保护新内容，无法再自动撤销。",
            false,
        ),
        "library_lock_poisoned" => (code, "本地题库暂时无法读取；请重启应用后重试。", true),
        _ => (
            "capture_recognition_failed",
            "本次识别没有完成；原图和手工分组保持不变。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!(
            "capture recognition operation failed [{diagnostic_id}] {}",
            error.code()
        );
    }
    AppResult::failure(stable_code, user_message, retryable, diagnostic_id)
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
    fn gate_closed_is_explanatory_and_not_retryable() {
        let AppResult::Failure { error, .. } =
            recognition_failure::<CaptureRecognitionJob>("capture_recognition_gate_closed", None)
        else {
            panic!("gate closed must be a failure")
        };
        assert_eq!(error.code, "capture_recognition_gate_closed");
        assert!(!error.retryable);
        assert!(error.user_message.contains("顺序模板"));
    }
}
