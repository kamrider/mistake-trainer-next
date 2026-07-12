use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::{
        capture::{CaptureStage, StageCaptureError, StagedAsset, stage_image_bytes},
        problems::{AssetRole, CaptureAsset, CreateProblem, create_problem},
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommitInput {
    pub subject: String,
    pub note: String,
    pub staged_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommitOutput {
    pub problem_id: String,
}

pub fn capture_list_for(stage: &CaptureStage) -> AppResult<Vec<StagedAsset>> {
    match stage.summaries() {
        Ok(assets) => AppResult::success(assets),
        Err(_) => capture_error(
            "capture_stage_unavailable",
            "图片暂存区暂时不可用，请重新打开应用后再试。",
            true,
        ),
    }
}

pub fn capture_remove_for(stage: &CaptureStage, staged_asset_id: &str) -> AppResult<bool> {
    match stage.remove(staged_asset_id) {
        Ok(removed) => AppResult::success(removed),
        Err(_) => capture_error(
            "capture_stage_unavailable",
            "图片暂存区暂时不可用，请重新打开应用后再试。",
            true,
        ),
    }
}

pub fn capture_commit_for(
    runtime: &LibraryRuntime,
    stage: &CaptureStage,
    input: CaptureCommitInput,
    now_utc_ms: i64,
) -> AppResult<CaptureCommitOutput> {
    let captures = match stage.captures(&input.staged_asset_ids) {
        Ok(captures) => captures,
        Err(StageCaptureError::NotFound) => {
            return capture_error(
                "staged_asset_missing",
                "有图片已经不在暂存区，请重新选择后再保存。",
                false,
            );
        }
        Err(_) => {
            return capture_error(
                "capture_stage_unavailable",
                "图片暂存区暂时不可用，请重新打开应用后再试。",
                true,
            );
        }
    };
    let has_question = captures
        .iter()
        .any(|capture| capture.summary.role == "question");
    let has_answer = captures
        .iter()
        .any(|capture| capture.summary.role == "answer");
    if !has_question || !has_answer {
        return capture_error(
            "question_and_answer_required",
            "请至少添加一张题图和一张答案图。",
            false,
        );
    }

    let assets = captures
        .into_iter()
        .map(|capture| CaptureAsset {
            role: if capture.summary.role == "question" {
                AssetRole::Question
            } else {
                AssetRole::Answer
            },
            media_type: capture.summary.media_type,
            bytes: capture.bytes,
        })
        .collect();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => {
            return capture_error(
                "library_lock_poisoned",
                "本地题库暂时不可写入，请重新打开应用后再试。",
                true,
            );
        }
    };
    let problem = match create_problem(
        &mut connection,
        &runtime.blob_root,
        &runtime.asset_key,
        CreateProblem {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.profile_id().to_owned(),
            subject: input.subject,
            note: input.note,
            assets,
            now_utc_ms,
        },
    ) {
        Ok(problem) => problem,
        Err(_) => {
            return capture_error(
                "capture_commit_failed",
                "错题没有保存成功，原图片仍在暂存区，可以稍后重试。",
                true,
            );
        }
    };
    drop(connection);
    if stage.remove_many(&input.staged_asset_ids).is_err() {
        return capture_error(
            "capture_cleanup_failed",
            "错题已保存，但暂存区未能清理；重新打开页面即可。",
            false,
        );
    }
    AppResult::success(CaptureCommitOutput {
        problem_id: problem.id,
    })
}

#[tauri::command]
#[specta::specta]
pub fn capture_commit(
    library: State<'_, LibraryRuntime>,
    stage: State<'_, CaptureStage>,
    input: CaptureCommitInput,
) -> AppResult<CaptureCommitOutput> {
    capture_commit_for(&library, &stage, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn capture_list(stage: State<'_, CaptureStage>) -> AppResult<Vec<StagedAsset>> {
    capture_list_for(&stage)
}

#[tauri::command]
#[specta::specta]
pub fn capture_remove(stage: State<'_, CaptureStage>, staged_asset_id: String) -> AppResult<bool> {
    capture_remove_for(&stage, &staged_asset_id)
}

#[tauri::command]
#[specta::specta]
pub fn capture_select(stage: State<'_, CaptureStage>, role: String) -> AppResult<Vec<StagedAsset>> {
    if !matches!(role.as_str(), "question" | "answer") {
        return capture_error(
            "capture_role_invalid",
            "图片类型无效，请重新选择题图或答案图。",
            false,
        );
    }
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .set_title(if role == "question" {
            "选择题目图片"
        } else {
            "选择答案图片"
        })
        .pick_files()
    else {
        return AppResult::success(Vec::new());
    };

    let mut selected = Vec::with_capacity(paths.len());
    for path in paths {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                let ids = selected
                    .iter()
                    .map(|asset: &StagedAsset| asset.id.clone())
                    .collect::<Vec<_>>();
                let _ = stage.remove_many(&ids);
                return capture_error(
                    "capture_image_unreadable",
                    "所选图片无法读取，请检查文件是否仍然存在。",
                    true,
                );
            }
        };
        match stage_image_bytes(&stage, file_name, &role, bytes) {
            Ok(asset) => selected.push(asset),
            Err(_) => {
                let ids = selected
                    .iter()
                    .map(|asset: &StagedAsset| asset.id.clone())
                    .collect::<Vec<_>>();
                let _ = stage.remove_many(&ids);
                return capture_error(
                    "capture_image_invalid",
                    "所选文件中有无法读取的图片；支持 PNG、JPEG 和 WebP，单张不超过 25 MB。",
                    false,
                );
            }
        }
    }
    AppResult::success(selected)
}

fn capture_error<T>(code: &str, message: &str, retryable: bool) -> AppResult<T> {
    AppResult::failure(code, message, retryable, Uuid::now_v7().to_string())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
