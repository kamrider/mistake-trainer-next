use std::{
    io::{Cursor, Read},
    path::{Component, Path},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{application::ports::assets::AssetDecryptor, modules::capture::MAX_CAPTURE_FILE_BYTES};

use super::{
    ProblemAssetPreview, ProblemDetail, ProblemDetailQuery, ProblemListQuery, ProblemSummary,
    ProblemUseCaseError,
};

const MAX_ENCRYPTED_ASSET_BYTES: u64 = MAX_CAPTURE_FILE_BYTES + 64;
const PREVIEW_MAX_DIMENSION: u32 = 1_600;
const LIST_PREVIEW_MAX_DIMENSION: u32 = 480;
const MAX_SOURCE_DIMENSION: u32 = 12_000;
const MAX_SOURCE_PIXELS: u64 = 80_000_000;

pub(super) fn list_problem_summaries(
    connection: &Connection,
    query: ProblemListQuery,
) -> Result<Vec<ProblemSummary>, ProblemUseCaseError> {
    list_problem_summaries_internal(connection, None, query)
}

pub(super) fn list_problem_summaries_with_previews(
    connection: &Connection,
    blob_root: &Path,
    asset_decryptor: &dyn AssetDecryptor,
    query: ProblemListQuery,
) -> Result<Vec<ProblemSummary>, ProblemUseCaseError> {
    list_problem_summaries_internal(connection, Some((blob_root, asset_decryptor)), query)
}

fn list_problem_summaries_internal(
    connection: &Connection,
    preview_store: Option<(&Path, &dyn AssetDecryptor)>,
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
    let account_id = query.account_id;
    let profile_id = query.profile_id;
    let now_utc_ms = query.now_utc_ms;
    let input = query.input.validated()?;
    let search = input
        .search
        .unwrap_or_default()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let subjects_json = serde_json::to_string(&input.subjects)?;
    let tags_json = serde_json::to_string(&input.tags)?;
    let recently_forgotten_after_utc_ms = now_utc_ms.saturating_sub(30 * 86_400_000);
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
           AND (?5 = '[]' OR EXISTS (
                SELECT 1 FROM json_each(?5) selected_subject
                WHERE CAST(selected_subject.value AS TEXT) = p.subject
           ))
           AND (?6 = '[]' OR EXISTS (
                SELECT 1
                FROM json_each(?6) selected_tag
                JOIN json_each(p.tags_json) problem_tag
                  ON CAST(problem_tag.value AS TEXT) = CAST(selected_tag.value AS TEXT)
           ))
           AND (
                ?7 = 'any'
                OR (?7 = 'never_reviewed' AND NOT EXISTS (
                    SELECT 1 FROM review_events review
                    WHERE review.account_id = p.account_id
                      AND review.profile_id = p.profile_id
                      AND review.problem_id = p.id
                ))
                OR (?7 = 'due' AND EXISTS (
                    SELECT 1 FROM schedule_states schedule
                    WHERE schedule.problem_id = p.id
                      AND schedule.due_at_utc_ms <= ?9
                ))
                OR (?7 = 'recently_forgotten' AND (
                    SELECT review.rating FROM review_events review
                    WHERE review.account_id = p.account_id
                      AND review.profile_id = p.profile_id
                      AND review.problem_id = p.id
                    ORDER BY review.occurred_at_utc_ms DESC, review.id DESC
                    LIMIT 1
                ) = 'again' AND (
                    SELECT review.occurred_at_utc_ms FROM review_events review
                    WHERE review.account_id = p.account_id
                      AND review.profile_id = p.profile_id
                      AND review.problem_id = p.id
                    ORDER BY review.occurred_at_utc_ms DESC, review.id DESC
                    LIMIT 1
                ) >= ?10)
           )
           AND (
                ?8 = 'any'
                OR (?8 = 'has_answer' AND EXISTS (
                    SELECT 1 FROM problem_assets answer_asset
                    WHERE answer_asset.problem_id = p.id AND answer_asset.role = 'answer'
                ))
                OR (?8 = 'missing_answer' AND NOT EXISTS (
                    SELECT 1 FROM problem_assets answer_asset
                    WHERE answer_asset.problem_id = p.id AND answer_asset.role = 'answer'
                ))
           )
         GROUP BY p.id
         ORDER BY p.updated_at_utc_ms DESC, p.id DESC",
    )?;
    let rows = statement.query_map(
        params![
            account_id,
            profile_id,
            input.status.as_str(),
            search,
            subjects_json,
            tags_json,
            input.review_state.as_str(),
            input.answer_state.as_str(),
            now_utc_ms,
            recently_forgotten_after_utc_ms,
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
            let question_preview_data_url =
                preview_store.and_then(|(blob_root, asset_decryptor)| {
                    let encrypted_path = row.question_asset_path.as_deref()?;
                    let media_type = row.question_asset_media_type.as_deref()?;
                    let encrypted =
                        read_decrypted_asset(blob_root, asset_decryptor, encrypted_path).ok()?;
                    let (preview_media_type, preview_bytes) = make_preview_with_dimension(
                        &encrypted,
                        media_type,
                        LIST_PREVIEW_MAX_DIMENSION,
                    )
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

pub(super) fn get_problem_detail(
    connection: &Connection,
    blob_root: &Path,
    asset_decryptor: &dyn AssetDecryptor,
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
            let bytes = read_decrypted_asset(blob_root, asset_decryptor, &encrypted_path)?;
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
    asset_decryptor: &dyn AssetDecryptor,
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
    asset_decryptor
        .decrypt(&encrypted)
        .map_err(|_| ProblemUseCaseError::Crypto)
}

fn make_preview(
    bytes: &[u8],
    media_type: &str,
) -> Result<(&'static str, Vec<u8>), ProblemUseCaseError> {
    make_preview_with_dimension(bytes, media_type, PREVIEW_MAX_DIMENSION)
}

fn make_preview_with_dimension(
    bytes: &[u8],
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
