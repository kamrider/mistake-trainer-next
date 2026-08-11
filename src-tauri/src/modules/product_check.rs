use std::{
    collections::HashMap,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Cursor, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::Mutex,
};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    domain::review::SimpleRating,
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::{
        backup::{create_backup, validate_backup},
        problems::{
            AssetRole, CaptureAsset, CreateProblem, ProblemAnswerState, ProblemDetailQuery,
            ProblemListInput, ProblemListQuery, ProblemReviewState, ProblemStatusFilter,
            create_problem, get_problem_detail, list_problem_summaries_with_previews,
        },
        review::{StartManualReview, SubmitReview, start_manual_review_queue, submit_review},
    },
};

const PRODUCT_CHECK_SCHEMA_VERSION: u32 = 1;
const WORKSPACE_PREFIX: &str = ".mistake-trainer-product-check-";
const SQLITE_PLAINTEXT_HEADER: &[u8; 16] = b"SQLite format 3\0";

#[derive(Debug, PartialEq, Eq)]
pub struct WindowsProductCheckRequest {
    pub output_path: PathBuf,
    pub scratch_root: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsProductCheckRequestError;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum WindowsProductCheckFailureCode {
    ScratchUnavailable,
    LibraryInitializeFailed,
    ProblemRoundTripFailed,
    ReviewRoundTripFailed,
    BackupValidationFailed,
    LibraryReopenFailed,
    CleanupFailed,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsProductChecks {
    encrypted_library: bool,
    problem_round_trip: bool,
    review_round_trip: bool,
    backup_validation: bool,
    library_reopen: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsProductCheckReport {
    schema_version: u32,
    application_version: String,
    checked_at_utc_ms: i64,
    ready: bool,
    failure_codes: Vec<WindowsProductCheckFailureCode>,
    checks: WindowsProductChecks,
}

#[derive(Default)]
struct EphemeralSecretStore(Mutex<HashMap<String, String>>);

impl SecretStore for EphemeralSecretStore {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        self.0
            .lock()
            .map(|values| values.get(name).cloned())
            .map_err(|_| "ephemeral secret store unavailable".to_owned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.0
            .lock()
            .map(|mut values| {
                values.insert(name.to_owned(), value.to_owned());
            })
            .map_err(|_| "ephemeral secret store unavailable".to_owned())
    }
}

pub fn parse_windows_product_check_request(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Option<WindowsProductCheckRequest>, WindowsProductCheckRequestError> {
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument != "--windows-product-check" {
            continue;
        }
        let output_path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or(WindowsProductCheckRequestError)?;
        let scratch_root = arguments
            .next()
            .map(PathBuf::from)
            .ok_or(WindowsProductCheckRequestError)?;
        if !output_path.is_absolute() || !scratch_root.is_absolute() {
            return Err(WindowsProductCheckRequestError);
        }
        return Ok(Some(WindowsProductCheckRequest {
            output_path,
            scratch_root,
        }));
    }
    Ok(None)
}

pub fn write_windows_product_check(
    output_path: &Path,
    scratch_root: &Path,
    application_version: &str,
    checked_at_utc_ms: i64,
) -> std::io::Result<bool> {
    let mut checks = WindowsProductChecks::default();
    let mut failure_codes = Vec::new();
    let workspace = prepare_workspace(scratch_root);

    match workspace.as_deref() {
        Some(workspace) => {
            if let Err(code) = run_product_checks(workspace, checked_at_utc_ms, &mut checks) {
                failure_codes.push(code);
            }
            if !remove_owned_workspace(scratch_root, workspace) {
                failure_codes.push(WindowsProductCheckFailureCode::CleanupFailed);
            }
        }
        None => failure_codes.push(WindowsProductCheckFailureCode::ScratchUnavailable),
    }

    let report = WindowsProductCheckReport {
        schema_version: PRODUCT_CHECK_SCHEMA_VERSION,
        application_version: application_version.to_owned(),
        checked_at_utc_ms,
        ready: failure_codes.is_empty(),
        failure_codes,
        checks,
    };
    let ready = report.ready;
    let bytes = serde_json::to_vec_pretty(&report).map_err(std::io::Error::other)?;
    write_new_synced(output_path, &bytes)?;
    Ok(ready)
}

fn prepare_workspace(scratch_root: &Path) -> Option<PathBuf> {
    let scratch_root = scratch_root.canonicalize().ok()?;
    if !scratch_root.is_dir() {
        return None;
    }
    let workspace = scratch_root.join(format!("{WORKSPACE_PREFIX}{}", Uuid::now_v7().simple()));
    fs::create_dir(&workspace).ok()?;
    Some(workspace)
}

fn remove_owned_workspace(scratch_root: &Path, workspace: &Path) -> bool {
    let Ok(scratch_root) = scratch_root.canonicalize() else {
        return false;
    };
    let Some(name) = workspace.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if workspace.parent() != Some(scratch_root.as_path()) || !name.starts_with(WORKSPACE_PREFIX) {
        return false;
    }
    fs::remove_dir_all(workspace).is_ok()
}

fn run_product_checks(
    workspace: &Path,
    checked_at_utc_ms: i64,
    checks: &mut WindowsProductChecks,
) -> Result<(), WindowsProductCheckFailureCode> {
    let library_root = workspace.join("library");
    let backup_root = workspace.join("backups");
    fs::create_dir(&backup_root)
        .map_err(|_| WindowsProductCheckFailureCode::LibraryInitializeFailed)?;
    let secrets = EphemeralSecretStore::default();
    let runtime = initialize_local_library(&library_root, &secrets, checked_at_utc_ms)
        .map_err(|_| WindowsProductCheckFailureCode::LibraryInitializeFailed)?;
    ensure_database_is_encrypted(&library_root.join("library.db"))
        .map_err(|_| WindowsProductCheckFailureCode::LibraryInitializeFailed)?;
    checks.encrypted_library = true;

    let profile = runtime.active_profile();
    let question = make_png([28, 89, 214, 255])
        .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
    let answer = make_png([34, 139, 94, 255])
        .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
    let problem = {
        let mut connection = runtime
            .connection
            .lock()
            .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
        create_problem(
            &mut connection,
            &runtime.blob_root,
            &runtime.asset_key,
            CreateProblem {
                account_id: runtime.account_id().to_owned(),
                profile_id: profile.id.clone(),
                subject: "安装包黄金路径".to_owned(),
                note: "隔离产品检查".to_owned(),
                assets: vec![
                    CaptureAsset {
                        role: AssetRole::Question,
                        media_type: "image/png".to_owned(),
                        bytes: question,
                    },
                    CaptureAsset {
                        role: AssetRole::Answer,
                        media_type: "image/png".to_owned(),
                        bytes: answer,
                    },
                ],
                now_utc_ms: checked_at_utc_ms + 1,
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?
    };
    {
        let connection = runtime
            .connection
            .lock()
            .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
        let summaries = list_problem_summaries_with_previews(
            &connection,
            &runtime.blob_root,
            &runtime.asset_key,
            ProblemListQuery {
                account_id: runtime.account_id().to_owned(),
                profile_id: profile.id.clone(),
                now_utc_ms: checked_at_utc_ms,
                input: ProblemListInput {
                    status: ProblemStatusFilter::Active,
                    search: Some("黄金路径".to_owned()),
                    subjects: vec![],
                    tags: vec![],
                    review_state: ProblemReviewState::Any,
                    answer_state: ProblemAnswerState::Any,
                },
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
        let detail = get_problem_detail(
            &connection,
            &runtime.blob_root,
            &runtime.asset_key,
            ProblemDetailQuery {
                account_id: runtime.account_id().to_owned(),
                profile_id: profile.id.clone(),
                problem_id: problem.id.clone(),
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::ProblemRoundTripFailed)?;
        if summaries.len() != 1
            || summaries[0].id != problem.id
            || summaries[0].question_asset_count != 1
            || summaries[0].answer_asset_count != 1
            || summaries[0].question_preview_data_url.is_none()
            || detail.assets.len() != 2
            || detail
                .assets
                .iter()
                .any(|asset| !asset.data_url.starts_with("data:image/png;base64,"))
        {
            return Err(WindowsProductCheckFailureCode::ProblemRoundTripFailed);
        }
    }
    checks.problem_round_trip = true;

    {
        let mut connection = runtime
            .connection
            .lock()
            .map_err(|_| WindowsProductCheckFailureCode::ReviewRoundTripFailed)?;
        let queue = start_manual_review_queue(
            &mut connection,
            StartManualReview {
                account_id: runtime.account_id().to_owned(),
                profile_id: profile.id.clone(),
                problem_ids: vec![problem.id.clone()],
                now_utc_ms: checked_at_utc_ms + 2,
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::ReviewRoundTripFailed)?;
        if queue.total_count != 1
            || queue.items.len() != 1
            || queue.items[0].problem_id != problem.id
        {
            return Err(WindowsProductCheckFailureCode::ReviewRoundTripFailed);
        }
        let submission = submit_review(
            &mut connection,
            SubmitReview {
                account_id: runtime.account_id().to_owned(),
                profile_id: profile.id.clone(),
                problem_id: problem.id.clone(),
                device_id: runtime.device_id().to_owned(),
                rating: SimpleRating::Remembered.into_fsrs(),
                duration_ms: 1_250,
                occurred_at_utc_ms: checked_at_utc_ms + 3,
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::ReviewRoundTripFailed)?;
        if submission.problem_id != problem.id || submission.rating != "good" {
            return Err(WindowsProductCheckFailureCode::ReviewRoundTripFailed);
        }
    }
    checks.review_round_trip = true;

    let backup = create_backup(
        runtime.connection.as_ref(),
        &runtime.blob_root,
        runtime.database_key(),
        runtime.account_id(),
        &backup_root,
        checked_at_utc_ms + 4,
    )
    .map_err(|_| WindowsProductCheckFailureCode::BackupValidationFailed)?;
    let validated = validate_backup(
        &backup_root.join(&backup.label),
        runtime.database_key(),
        &runtime.asset_key,
        runtime.account_id(),
    )
    .map_err(|_| WindowsProductCheckFailureCode::BackupValidationFailed)?;
    if validated.asset_count != 2 || !validated.ready_for_restore {
        return Err(WindowsProductCheckFailureCode::BackupValidationFailed);
    }
    checks.backup_validation = true;

    drop(runtime);
    let reopened = initialize_local_library(&library_root, &secrets, checked_at_utc_ms + 5)
        .map_err(|_| WindowsProductCheckFailureCode::LibraryReopenFailed)?;
    {
        let connection = reopened
            .connection
            .lock()
            .map_err(|_| WindowsProductCheckFailureCode::LibraryReopenFailed)?;
        let persisted: (i64, i64) = connection
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM problems WHERE account_id = ?1),
                    (SELECT COUNT(*) FROM review_events WHERE account_id = ?1)",
                [reopened.account_id()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| WindowsProductCheckFailureCode::LibraryReopenFailed)?;
        let detail = get_problem_detail(
            &connection,
            &reopened.blob_root,
            &reopened.asset_key,
            ProblemDetailQuery {
                account_id: reopened.account_id().to_owned(),
                profile_id: reopened.active_profile().id,
                problem_id: problem.id,
            },
        )
        .map_err(|_| WindowsProductCheckFailureCode::LibraryReopenFailed)?;
        if persisted != (1, 1) || detail.assets.len() != 2 {
            return Err(WindowsProductCheckFailureCode::LibraryReopenFailed);
        }
    }
    checks.library_reopen = true;
    Ok(())
}

fn make_png(color: [u8; 4]) -> Result<Vec<u8>, image::ImageError> {
    let image = RgbaImage::from_pixel(2, 2, Rgba(color));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

fn ensure_database_is_encrypted(path: &Path) -> std::io::Result<()> {
    let mut input = fs::File::open(path)?;
    let mut header = [0_u8; SQLITE_PLAINTEXT_HEADER.len()];
    input.read_exact(&mut header)?;
    if &header == SQLITE_PLAINTEXT_HEADER {
        return Err(std::io::Error::other("database header is plaintext"));
    }
    Ok(())
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    output.write_all(bytes)?;
    output.sync_all()
}
