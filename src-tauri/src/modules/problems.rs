use std::{
    collections::{BTreeSet, HashMap},
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::assets::{AssetCryptoError, decrypt_asset, encrypt_asset, plaintext_sha256},
    modules::capture::MAX_CAPTURE_FILE_BYTES,
};

const MAX_ENCRYPTED_ASSET_BYTES: u64 = MAX_CAPTURE_FILE_BYTES + 64;
const PREVIEW_MAX_DIMENSION: u32 = 1_600;
const LIST_PREVIEW_MAX_DIMENSION: u32 = 480;
const MAX_SOURCE_DIMENSION: u32 = 12_000;
const MAX_SOURCE_PIXELS: u64 = 80_000_000;
const TRASH_RETENTION_MILLIS: i64 = 30 * 86_400_000;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    Question,
    Answer,
}

impl AssetRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Answer => "answer",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CaptureAsset {
    pub role: AssetRole,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct CreateProblem {
    pub account_id: String,
    pub profile_id: String,
    pub subject: String,
    pub note: String,
    pub assets: Vec<CaptureAsset>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub id: String,
    pub account_id: String,
    pub profile_id: String,
    pub subject: String,
    pub note: String,
    pub asset_ids: Vec<String>,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProblemStatusFilter {
    Active,
    Archived,
    Trashed,
}

impl ProblemStatusFilter {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Trashed => "trashed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProblemListQuery {
    pub account_id: String,
    pub profile_id: String,
    pub status: ProblemStatusFilter,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemSummary {
    pub id: String,
    pub subject: String,
    pub note: String,
    pub tags: Vec<String>,
    pub status: String,
    pub question_asset_count: i32,
    pub answer_asset_count: i32,
    pub question_preview_data_url: Option<String>,
    pub updated_at_utc_ms: f64,
}

#[derive(Clone, Debug)]
pub struct ProblemDetailQuery {
    pub account_id: String,
    pub profile_id: String,
    pub problem_id: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemAssetPreview {
    pub id: String,
    pub role: String,
    pub position: i32,
    pub media_type: String,
    pub data_url: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetail {
    pub id: String,
    pub subject: String,
    pub note: String,
    pub tags: Vec<String>,
    pub status: String,
    pub time_limit_seconds: Option<i32>,
    pub updated_at_utc_ms: f64,
    pub assets: Vec<ProblemAssetPreview>,
}

#[derive(Clone, Debug)]
pub struct UpdateProblem {
    pub account_id: String,
    pub profile_id: String,
    pub problem_id: String,
    pub subject: String,
    pub note: String,
    pub tags: Vec<String>,
    pub time_limit_seconds: Option<i32>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ChangeProblemStatus {
    pub account_id: String,
    pub profile_id: String,
    pub problem_ids: Vec<String>,
    pub target_status: ProblemStatusFilter,
    pub now_utc_ms: i64,
}

#[derive(Debug, Error)]
pub enum ProblemUseCaseError {
    #[error("learner profile was not found for this account")]
    ProfileNotFound,
    #[error("a problem requires at least one non-empty asset")]
    MissingAsset,
    #[error("problem persistence failed")]
    Database(#[from] rusqlite::Error),
    #[error("problem was not found for this account and profile")]
    ProblemNotFound,
    #[error("stored asset path is invalid")]
    InvalidAssetPath,
    #[error("stored asset is too large")]
    AssetTooLarge,
    #[error("stored asset image is invalid")]
    InvalidAssetImage,
    #[error("problem text is too long")]
    InvalidText,
    #[error("problem tags exceed the allowed count or length")]
    InvalidTags,
    #[error("problem time limit must be between 1 and 86400 seconds")]
    InvalidTimeLimit,
    #[error("at least one problem must be selected")]
    EmptySelection,
    #[error("asset encryption failed")]
    Crypto(#[from] AssetCryptoError),
    #[error("asset file operation failed")]
    File(#[from] std::io::Error),
    #[error("problem outbox serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("problem has unresolved sync conflicts")]
    ConflictPending,
}

pub fn list_problem_summaries(
    connection: &Connection,
    query: ProblemListQuery,
) -> Result<Vec<ProblemSummary>, ProblemUseCaseError> {
    list_problem_summaries_internal(connection, None, query)
}

pub fn list_problem_summaries_with_previews(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    query: ProblemListQuery,
) -> Result<Vec<ProblemSummary>, ProblemUseCaseError> {
    list_problem_summaries_internal(connection, Some((blob_root, key)), query)
}

fn list_problem_summaries_internal(
    connection: &Connection,
    preview_store: Option<(&Path, &[u8; 32])>,
    query: ProblemListQuery,
) -> Result<Vec<ProblemSummary>, ProblemUseCaseError> {
    struct ProblemSummaryRow {
        id: String,
        subject: String,
        note: String,
        tags_json: String,
        status: String,
        question_asset_count: i32,
        answer_asset_count: i32,
        updated_at_utc_ms: f64,
        question_asset_path: Option<String>,
        question_asset_media_type: Option<String>,
    }
    let search = query
        .search
        .unwrap_or_default()
        .trim()
        .chars()
        .take(100)
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let mut statement = connection.prepare(
        "SELECT p.id, p.subject, p.note, p.tags_json, p.status,
                SUM(CASE WHEN pa.role = 'question' THEN 1 ELSE 0 END),
                SUM(CASE WHEN pa.role = 'answer' THEN 1 ELSE 0 END),
                p.updated_at_utc_ms,
                (SELECT a.encrypted_path
                 FROM problem_assets pa_preview
                 JOIN assets a ON a.id = pa_preview.asset_id
                 WHERE pa_preview.problem_id = p.id
                   AND pa_preview.role = 'question'
                   AND a.account_id = p.account_id
                 ORDER BY pa_preview.position
                 LIMIT 1),
                (SELECT a.media_type
                 FROM problem_assets pa_preview
                 JOIN assets a ON a.id = pa_preview.asset_id
                 WHERE pa_preview.problem_id = p.id
                   AND pa_preview.role = 'question'
                   AND a.account_id = p.account_id
                 ORDER BY pa_preview.position
                 LIMIT 1)
         FROM problems p
         LEFT JOIN problem_assets pa ON pa.problem_id = p.id
         WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = ?3
           AND (?4 = '' OR p.subject LIKE '%' || ?4 || '%' ESCAPE '\\'
                        OR p.note LIKE '%' || ?4 || '%' ESCAPE '\\'
                        OR EXISTS (
                            SELECT 1 FROM json_each(p.tags_json) tag
                            WHERE CAST(tag.value AS TEXT) LIKE '%' || ?4 || '%' ESCAPE '\\'
                        ))
         GROUP BY p.id
         ORDER BY p.updated_at_utc_ms DESC, p.id DESC",
    )?;
    let rows = statement.query_map(
        params![
            query.account_id,
            query.profile_id,
            query.status.as_str(),
            search
        ],
        |row| {
            Ok(ProblemSummaryRow {
                id: row.get(0)?,
                subject: row.get(1)?,
                note: row.get(2)?,
                tags_json: row.get(3)?,
                status: row.get(4)?,
                question_asset_count: row.get(5)?,
                answer_asset_count: row.get(6)?,
                updated_at_utc_ms: row.get(7)?,
                question_asset_path: row.get(8)?,
                question_asset_media_type: row.get(9)?,
            })
        },
    )?;

    rows.collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|row| {
            let tags = serde_json::from_str::<Vec<String>>(&row.tags_json)?;
            let question_preview_data_url = preview_store.and_then(|(blob_root, key)| {
                let encrypted_path = row.question_asset_path.as_deref()?;
                let media_type = row.question_asset_media_type.as_deref()?;
                let encrypted = read_decrypted_asset(blob_root, key, encrypted_path).ok()?;
                let (preview_media_type, preview_bytes) =
                    make_preview_with_dimension(&encrypted, media_type, LIST_PREVIEW_MAX_DIMENSION)
                        .ok()?;
                Some(format!(
                    "data:{preview_media_type};base64,{}",
                    STANDARD.encode(preview_bytes)
                ))
            });
            Ok(ProblemSummary {
                id: row.id,
                subject: row.subject,
                note: row.note,
                tags,
                status: row.status,
                question_asset_count: row.question_asset_count,
                answer_asset_count: row.answer_asset_count,
                question_preview_data_url,
                updated_at_utc_ms: row.updated_at_utc_ms,
            })
        })
        .collect()
}

pub fn get_problem_detail(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    query: ProblemDetailQuery,
) -> Result<ProblemDetail, ProblemUseCaseError> {
    let detail_row = connection
        .query_row(
            "SELECT id, subject, note, tags_json, status, time_limit_seconds, updated_at_utc_ms
             FROM problems
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![query.problem_id, query.account_id, query.profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i32>>(5)?,
                    row.get::<_, f64>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or(ProblemUseCaseError::ProblemNotFound)?;
    let mut detail = ProblemDetail {
        id: detail_row.0,
        subject: detail_row.1,
        note: detail_row.2,
        tags: serde_json::from_str(&detail_row.3)?,
        status: detail_row.4,
        time_limit_seconds: detail_row.5,
        updated_at_utc_ms: detail_row.6,
        assets: Vec::new(),
    };

    let mut statement = connection.prepare(
        "SELECT a.id, pa.role, pa.position, a.media_type, a.encrypted_path
         FROM problem_assets pa
         JOIN assets a ON a.id = pa.asset_id
         WHERE pa.problem_id = ?1 AND a.account_id = ?2
         ORDER BY CASE pa.role WHEN 'question' THEN 0 ELSE 1 END, pa.position",
    )?;
    let rows = statement
        .query_map(params![detail.id, query.account_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    detail.assets = rows
        .into_iter()
        .map(|(id, role, position, media_type, encrypted_path)| {
            let bytes = read_decrypted_asset(blob_root, key, &encrypted_path)?;
            let (preview_media_type, preview_bytes) = make_preview(&bytes, &media_type)?;
            Ok(ProblemAssetPreview {
                id,
                role,
                position,
                media_type: preview_media_type.to_owned(),
                data_url: format!(
                    "data:{preview_media_type};base64,{}",
                    STANDARD.encode(preview_bytes)
                ),
            })
        })
        .collect::<Result<Vec<_>, ProblemUseCaseError>>()?;
    Ok(detail)
}

fn read_decrypted_asset(
    blob_root: &Path,
    key: &[u8; 32],
    encrypted_path: &str,
) -> Result<Vec<u8>, ProblemUseCaseError> {
    let relative = Path::new(encrypted_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ProblemUseCaseError::InvalidAssetPath);
    }
    let file = std::fs::File::open(blob_root.join(relative))?;
    let mut reader = file.take(MAX_ENCRYPTED_ASSET_BYTES + 1);
    let mut encrypted = Vec::new();
    reader.read_to_end(&mut encrypted)?;
    if u64::try_from(encrypted.len()).unwrap_or(u64::MAX) > MAX_ENCRYPTED_ASSET_BYTES {
        return Err(ProblemUseCaseError::AssetTooLarge);
    }
    decrypt_asset(&encrypted, key).map_err(ProblemUseCaseError::Crypto)
}

fn make_preview<'a>(
    bytes: &'a [u8],
    media_type: &str,
) -> Result<(&'static str, Vec<u8>), ProblemUseCaseError> {
    make_preview_with_dimension(bytes, media_type, PREVIEW_MAX_DIMENSION)
}

fn make_preview_with_dimension<'a>(
    bytes: &'a [u8],
    media_type: &str,
    max_dimension: u32,
) -> Result<(&'static str, Vec<u8>), ProblemUseCaseError> {
    let format = match media_type {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        _ => return Err(ProblemUseCaseError::InvalidAssetImage),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| ProblemUseCaseError::InvalidAssetImage)?;
    if width == 0
        || height == 0
        || width > MAX_SOURCE_DIMENSION
        || height > MAX_SOURCE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS
    {
        return Err(ProblemUseCaseError::InvalidAssetImage);
    }
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|_| ProblemUseCaseError::InvalidAssetImage)?;
    if image.width() <= max_dimension && image.height() <= max_dimension {
        return Ok((media_type_for(format), bytes.to_vec()));
    }
    let thumbnail = image.thumbnail(max_dimension, max_dimension);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| ProblemUseCaseError::InvalidAssetImage)?;
    Ok(("image/png", output.into_inner()))
}

const fn media_type_for(format: image::ImageFormat) -> &'static str {
    match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn update_problem(
    connection: &mut Connection,
    input: UpdateProblem,
) -> Result<(), ProblemUseCaseError> {
    let subject = input.subject.trim();
    let note = input.note.trim();
    if subject.chars().count() > 40 || note.chars().count() > 2_000 {
        return Err(ProblemUseCaseError::InvalidText);
    }
    if input.tags.len() > 20 {
        return Err(ProblemUseCaseError::InvalidTags);
    }
    let mut seen = BTreeSet::new();
    let tags = input
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            if tag.chars().count() > 30 {
                Err(ProblemUseCaseError::InvalidTags)
            } else {
                Ok(tag)
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|tag| seen.insert(tag.clone()))
        .collect::<Vec<_>>();
    let tags_json = serde_json::to_string(&tags)?;
    if input
        .time_limit_seconds
        .is_some_and(|seconds| !(1..=86_400).contains(&seconds))
    {
        return Err(ProblemUseCaseError::InvalidTimeLimit);
    }
    let transaction = connection.transaction()?;
    if crate::modules::sync_conflicts::has_open_conflict(
        &transaction,
        &input.account_id,
        "problem",
        &input.problem_id,
    )? {
        return Err(ProblemUseCaseError::ConflictPending);
    }
    let base_revision = transaction
        .query_row(
            "SELECT revision FROM problems WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.problem_id, input.account_id, input.profile_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(ProblemUseCaseError::ProblemNotFound)?;
    let new_revision = base_revision + 1;
    let changed = transaction.execute(
        "UPDATE problems
         SET subject = ?1, note = ?2, tags_json = ?3, time_limit_seconds = ?4, updated_at_utc_ms = ?5, revision = ?6
         WHERE id = ?7 AND account_id = ?8 AND profile_id = ?9 AND revision = ?10",
        params![
            subject,
            note,
            tags_json,
            input.time_limit_seconds,
            input.now_utc_ms,
            new_revision,
            input.problem_id,
            input.account_id,
            input.profile_id,
            base_revision
        ],
    )?;
    if changed != 1 {
        return Err(ProblemUseCaseError::ProblemNotFound);
    }
    let payload = serde_json::to_string(&serde_json::json!({
        "id": input.problem_id,
        "subject": subject,
        "note": note,
        "tags": tags,
        "timeLimitSeconds": input.time_limit_seconds,
        "baseRevision": base_revision,
        "revision": new_revision,
        "updatedAtUtcMs": input.now_utc_ms,
    }))?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![
            Uuid::now_v7().to_string(),
            input.account_id,
            input.profile_id,
            input.problem_id,
            payload,
            input.now_utc_ms
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn change_problem_status(
    connection: &mut Connection,
    input: ChangeProblemStatus,
) -> Result<usize, ProblemUseCaseError> {
    let problem_ids = input.problem_ids.into_iter().collect::<BTreeSet<_>>();
    if problem_ids.is_empty() {
        return Err(ProblemUseCaseError::EmptySelection);
    }
    let transaction = connection.transaction()?;
    for problem_id in &problem_ids {
        if crate::modules::sync_conflicts::has_open_conflict(
            &transaction,
            &input.account_id,
            "problem",
            problem_id,
        )? {
            return Err(ProblemUseCaseError::ConflictPending);
        }
        let current = transaction
            .query_row(
                "SELECT status, revision FROM problems
                 WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
                params![problem_id, input.account_id, input.profile_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or(ProblemUseCaseError::ProblemNotFound)?;
        let next_revision = current.1 + 1;
        let target = input.target_status.as_str();
        transaction.execute(
            "UPDATE problems SET status = ?1, updated_at_utc_ms = ?2, revision = ?3 WHERE id = ?4",
            params![target, input.now_utc_ms, next_revision, problem_id],
        )?;
        let purge_after_utc_ms = input.now_utc_ms + TRASH_RETENTION_MILLIS;
        let operation = if target == "trashed" {
            transaction.execute(
                "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
                 VALUES(?1, ?2, ?3, 'problem', ?4, ?5, ?6, ?7)
                 ON CONFLICT(entity_type, entity_id) DO UPDATE SET deleted_at_utc_ms = excluded.deleted_at_utc_ms, purge_after_utc_ms = excluded.purge_after_utc_ms, revision = excluded.revision",
                params![
                    Uuid::now_v7().to_string(), input.account_id, input.profile_id, problem_id,
                    input.now_utc_ms, purge_after_utc_ms, next_revision
                ],
            )?;
            "delete"
        } else {
            transaction.execute(
                "DELETE FROM tombstones WHERE entity_type = 'problem' AND entity_id = ?1",
                [problem_id],
            )?;
            if current.0 == "trashed" {
                "restore"
            } else {
                "upsert"
            }
        };
        let payload = if target == "trashed" {
            serde_json::json!({
                "id": problem_id,
                "status": target,
                "baseRevision": current.1,
                "revision": next_revision,
                "deletedAtUtcMs": input.now_utc_ms,
                "purgeAfterUtcMs": purge_after_utc_ms,
            })
        } else {
            serde_json::json!({
                "id": problem_id,
                "status": target,
                "baseRevision": current.1,
                "revision": next_revision,
                "restoredAtUtcMs": if current.0 == "trashed" { Some(input.now_utc_ms) } else { None },
                "updatedAtUtcMs": input.now_utc_ms,
            })
        };
        let payload = serde_json::to_string(&payload)?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
             VALUES(?1, ?2, ?3, 'problem', ?4, ?5, ?6, 'pending', 0, ?7, ?7)",
            params![
                Uuid::now_v7().to_string(), input.account_id, input.profile_id, problem_id,
                operation, payload, input.now_utc_ms
            ],
        )?;
    }
    transaction.commit()?;
    Ok(problem_ids.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetMetadata {
    id: String,
    account_id: String,
    plaintext_sha256: String,
    encrypted_path: String,
    byte_length: i64,
    media_type: String,
    created_at_utc_ms: i64,
}

struct NewAsset {
    metadata: AssetMetadata,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
}

struct AssetLink {
    asset_id: String,
    role: AssetRole,
    position: i64,
}

pub fn create_problem(
    connection: &mut Connection,
    blob_root: &Path,
    key: &[u8; 32],
    input: CreateProblem,
) -> Result<Problem, ProblemUseCaseError> {
    let profile_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![input.profile_id, input.account_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(ProblemUseCaseError::ProfileNotFound);
    }
    if input.assets.is_empty() || input.assets.iter().any(|asset| asset.bytes.is_empty()) {
        return Err(ProblemUseCaseError::MissingAsset);
    }

    let mut known_assets = HashMap::<String, String>::new();
    let mut new_assets = Vec::<NewAsset>::new();
    let mut links = Vec::<AssetLink>::new();
    let mut question_position = 0_i64;
    let mut answer_position = 0_i64;
    let staging_root = blob_root.join(".staging");

    for capture in input.assets {
        let hash = plaintext_sha256(&capture.bytes);
        let asset_id = if let Some(id) = known_assets.get(&hash) {
            id.clone()
        } else if let Some(id) = connection
            .query_row(
                "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
                params![input.account_id, hash],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            known_assets.insert(hash.clone(), id.clone());
            id
        } else {
            let id = Uuid::now_v7().to_string();
            let shard = &id[..2];
            let relative = PathBuf::from("blobs").join(shard).join(format!("{id}.mtb"));
            let staged_path = staging_root.join(format!("{id}.tmp"));
            let final_path = blob_root.join(&relative);
            std::fs::create_dir_all(&staging_root)?;
            std::fs::write(&staged_path, encrypt_asset(&capture.bytes, key)?)?;
            new_assets.push(NewAsset {
                metadata: AssetMetadata {
                    id: id.clone(),
                    account_id: input.account_id.clone(),
                    plaintext_sha256: hash.clone(),
                    encrypted_path: relative.to_string_lossy().replace('\\', "/"),
                    byte_length: i64::try_from(capture.bytes.len()).unwrap_or(i64::MAX),
                    media_type: capture.media_type,
                    created_at_utc_ms: input.now_utc_ms,
                },
                staged_path,
                final_path,
                moved_to_final: false,
            });
            known_assets.insert(hash, id.clone());
            id
        };

        let position = match capture.role {
            AssetRole::Question => {
                let current = question_position;
                question_position += 1;
                current
            }
            AssetRole::Answer => {
                let current = answer_position;
                answer_position += 1;
                current
            }
        };
        links.push(AssetLink {
            asset_id,
            role: capture.role,
            position,
        });
    }

    let problem = Problem {
        id: Uuid::now_v7().to_string(),
        account_id: input.account_id,
        profile_id: input.profile_id,
        subject: input.subject.trim().to_owned(),
        note: input.note.trim().to_owned(),
        asset_ids: links.iter().map(|link| link.asset_id.clone()).collect(),
        created_at_utc_ms: input.now_utc_ms,
        updated_at_utc_ms: input.now_utc_ms,
        revision: 1,
    };

    let result = persist_problem(
        connection,
        &problem,
        &links,
        &mut new_assets,
        input.now_utc_ms,
    );
    if result.is_err() {
        cleanup_new_assets(&new_assets);
    }
    let _ = std::fs::remove_dir(&staging_root);
    result.map(|_| problem)
}

fn persist_problem(
    connection: &mut Connection,
    problem: &Problem,
    links: &[AssetLink],
    new_assets: &mut [NewAsset],
    now_utc_ms: i64,
) -> Result<(), ProblemUseCaseError> {
    let problem_payload = serde_json::to_string(problem)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, created_at_utc_ms, updated_at_utc_ms, revision) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![problem.id, problem.account_id, problem.profile_id, problem.subject, problem.note, problem.created_at_utc_ms, problem.updated_at_utc_ms, problem.revision],
    )?;

    for asset in new_assets.iter() {
        let metadata = &asset.metadata;
        transaction.execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![metadata.id, metadata.account_id, metadata.plaintext_sha256, metadata.encrypted_path, metadata.byte_length, metadata.media_type, metadata.created_at_utc_ms],
        )?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'asset', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
            params![Uuid::now_v7().to_string(), problem.account_id, problem.profile_id, metadata.id, serde_json::to_string(metadata)?, now_utc_ms],
        )?;
    }

    for link in links {
        transaction.execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position) VALUES(?1, ?2, ?3, ?4)",
            params![problem.id, link.asset_id, link.role.as_str(), link.position],
        )?;
    }
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), problem.account_id, problem.profile_id, problem.id, problem_payload, now_utc_ms],
    )?;

    for asset in new_assets.iter_mut() {
        if let Some(parent) = asset.final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&asset.staged_path, &asset.final_path)?;
        asset.moved_to_final = true;
    }

    transaction.commit()?;
    Ok(())
}

fn cleanup_new_assets(assets: &[NewAsset]) {
    for asset in assets {
        let path = if asset.moved_to_final {
            &asset.final_path
        } else {
            &asset.staged_path
        };
        let _ = std::fs::remove_file(path);
    }
}
