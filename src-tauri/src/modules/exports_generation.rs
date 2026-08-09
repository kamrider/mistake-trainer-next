use std::{
    fs::{File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

use docx_rs::{Docx, Paragraph, Pic, Run};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use uuid::Uuid;

use crate::infrastructure::assets::decrypt_asset;

use super::{ExportError, ExportLayout, GeneratedExportSummary};

const MAX_ENCRYPTED_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXPORT_PLAINTEXT_BYTES: usize = 256 * 1024 * 1024;
const MAX_EXPORT_ASSETS: usize = 2_000;
const MAX_DOCX_IMAGE_PIXELS: u64 = 40_000_000;
const MAX_DOCX_IMAGE_DIMENSION: u32 = 8_000;
const MAX_DOCX_TOTAL_PIXELS: u64 = 160_000_000;

#[derive(Deserialize)]
struct StoredConfiguration {
    layout: ExportLayout,
}

struct StoredSnapshot {
    id: String,
    title: String,
    problem_ids: Vec<String>,
    layout: ExportLayout,
}

struct ExportAsset {
    role: String,
    position: i32,
    media_type: String,
    encrypted_path: String,
    byte_length: usize,
}

struct ExportProblem {
    subject: String,
    note: String,
    assets: Vec<ExportAsset>,
}

pub(super) struct PreparedExport {
    snapshot: StoredSnapshot,
    problems: Vec<ExportProblem>,
    blob_root: PathBuf,
    asset_key: [u8; 32],
}

pub fn generate_export(
    connection: &Connection,
    blob_root: &Path,
    asset_key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
    destination: &Path,
) -> Result<GeneratedExportSummary, ExportError> {
    let prepared = prepare_export(
        connection,
        blob_root,
        asset_key,
        account_id,
        profile_id,
        snapshot_id,
    )?;
    write_prepared_export(prepared, destination)
}

pub(super) fn prepare_export(
    connection: &Connection,
    blob_root: &Path,
    asset_key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
) -> Result<PreparedExport, ExportError> {
    let snapshot = load_snapshot(connection, account_id, profile_id, snapshot_id)?;
    let problems = load_export_problems(connection, account_id, profile_id, &snapshot.problem_ids)?;
    Ok(PreparedExport {
        snapshot,
        problems,
        blob_root: blob_root.to_owned(),
        asset_key: *asset_key,
    })
}

pub(super) fn write_prepared_export(
    prepared: PreparedExport,
    destination: &Path,
) -> Result<GeneratedExportSummary, ExportError> {
    if !destination.is_absolute() {
        return Err(ExportError::InvalidDestination);
    }
    let destination = destination
        .canonicalize()
        .map_err(|_| ExportError::InvalidDestination)?;
    if !destination.is_dir() {
        return Err(ExportError::InvalidDestination);
    }
    let PreparedExport {
        snapshot,
        problems,
        blob_root,
        asset_key,
    } = prepared;
    let canonical_blob_root = blob_root
        .canonicalize()
        .map_err(|_| ExportError::InvalidAssetPath)?;
    let safe_title = safe_output_stem(&snapshot.title);
    let suffix = Uuid::now_v7().simple().to_string();
    let base_name = format!("{safe_title}-{suffix}");
    let output_name = match snapshot.layout {
        ExportLayout::OriginalImageFolder => generate_original_folder(
            &destination,
            &base_name,
            &problems,
            &canonical_blob_root,
            &asset_key,
        )?,
        ExportLayout::QuestionAnswerAlternating | ExportLayout::QuestionsThenAnswers => {
            validate_docx_assets(&problems, &canonical_blob_root, &asset_key)?;
            generate_docx(
                &destination,
                &base_name,
                &snapshot,
                &problems,
                &canonical_blob_root,
                &asset_key,
            )?
        }
    };
    Ok(GeneratedExportSummary {
        snapshot_id: snapshot.id,
        output_name,
        problem_count: i32::try_from(problems.len()).unwrap_or(i32::MAX),
        layout: snapshot.layout,
    })
}

fn load_snapshot(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
) -> Result<StoredSnapshot, ExportError> {
    let stored = connection
        .query_row(
            "SELECT s.title, s.problem_ids_json, s.configuration_json
             FROM export_snapshots s
             WHERE s.id = ?1 AND s.account_id = ?2 AND s.profile_id = ?3
               AND NOT EXISTS(
                   SELECT 1 FROM tombstones t
                   WHERE t.entity_type = 'export_snapshot' AND t.entity_id = s.id
               )",
            params![snapshot_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(ExportError::SnapshotNotFound)?;
    let problem_ids = serde_json::from_str::<Vec<String>>(&stored.1)?;
    let configuration = serde_json::from_str::<StoredConfiguration>(&stored.2)?;
    Ok(StoredSnapshot {
        id: snapshot_id.to_owned(),
        title: stored.0,
        problem_ids,
        layout: configuration.layout,
    })
}

fn load_export_problems(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    problem_ids: &[String],
) -> Result<Vec<ExportProblem>, ExportError> {
    let mut total_plaintext_bytes = 0_usize;
    let mut total_assets = 0_usize;
    problem_ids
        .iter()
        .map(|problem_id| {
            let (subject, note) = connection
                .query_row(
                    "SELECT subject, note FROM problems
                     WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND status != 'trashed'",
                    params![problem_id, account_id, profile_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or(ExportError::ProblemNotFound)?;
            let mut statement = connection.prepare(
                "SELECT pa.role, pa.position, a.media_type, a.encrypted_path, a.byte_length
                 FROM problem_assets pa
                 JOIN assets a ON a.id = pa.asset_id
                 WHERE pa.problem_id = ?1 AND a.account_id = ?2
                 ORDER BY CASE pa.role WHEN 'question' THEN 0 ELSE 1 END, pa.position",
            )?;
            let stored_assets = statement
                .query_map(params![problem_id, account_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let assets = stored_assets
                .into_iter()
                .map(
                    |(role, position, media_type, encrypted_path, byte_length)| {
                        total_assets = total_assets
                            .checked_add(1)
                            .filter(|total| *total <= MAX_EXPORT_ASSETS)
                            .ok_or(ExportError::ExportTooLarge)?;
                        let byte_length = usize::try_from(byte_length)
                            .map_err(|_| ExportError::ExportTooLarge)?;
                        total_plaintext_bytes = total_plaintext_bytes
                            .checked_add(byte_length)
                            .filter(|total| *total <= MAX_EXPORT_PLAINTEXT_BYTES)
                            .ok_or(ExportError::ExportTooLarge)?;
                        Ok(ExportAsset {
                            role,
                            position,
                            media_type,
                            encrypted_path,
                            byte_length,
                        })
                    },
                )
                .collect::<Result<Vec<_>, ExportError>>()?;
            Ok(ExportProblem {
                subject,
                note,
                assets,
            })
        })
        .collect()
}

fn read_decrypted_export_asset(
    blob_root: &Path,
    key: &[u8; 32],
    encrypted_path: &str,
) -> Result<Vec<u8>, ExportError> {
    let relative = Path::new(encrypted_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExportError::InvalidAssetPath);
    }
    let candidate = blob_root
        .join(relative)
        .canonicalize()
        .map_err(|_| ExportError::InvalidAssetPath)?;
    if !candidate.starts_with(blob_root) {
        return Err(ExportError::InvalidAssetPath);
    }
    let file = File::open(candidate)?;
    let mut encrypted = Vec::new();
    file.take(MAX_ENCRYPTED_ASSET_BYTES + 1)
        .read_to_end(&mut encrypted)?;
    if u64::try_from(encrypted.len()).unwrap_or(u64::MAX) > MAX_ENCRYPTED_ASSET_BYTES {
        return Err(ExportError::AssetTooLarge);
    }
    decrypt_asset(&encrypted, key).map_err(|_| ExportError::InvalidImage)
}

fn generate_original_folder(
    destination: &Path,
    base_name: &str,
    problems: &[ExportProblem],
    blob_root: &Path,
    asset_key: &[u8; 32],
) -> Result<String, ExportError> {
    let output_name = base_name.to_owned();
    let final_path = destination.join(&output_name);
    let temporary_path = destination.join(format!(".{}.tmp", Uuid::now_v7()));
    std::fs::create_dir(&temporary_path)?;
    let result = (|| {
        for (problem_index, problem) in problems.iter().enumerate() {
            for asset in &problem.assets {
                let bytes =
                    read_decrypted_export_asset(blob_root, asset_key, &asset.encrypted_path)?;
                if bytes.len() != asset.byte_length {
                    return Err(ExportError::InvalidImage);
                }
                let extension = media_extension(&asset.media_type)?;
                let file_name = format!(
                    "{:03}-{}-{:02}.{extension}",
                    problem_index + 1,
                    asset.role,
                    asset.position + 1
                );
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(temporary_path.join(file_name))?;
                file.write_all(&bytes)?;
                file.sync_all()?;
            }
        }
        std::fs::rename(&temporary_path, &final_path)?;
        Ok::<(), ExportError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&temporary_path);
    }
    result?;
    Ok(output_name)
}

fn generate_docx(
    destination: &Path,
    base_name: &str,
    snapshot: &StoredSnapshot,
    problems: &[ExportProblem],
    blob_root: &Path,
    asset_key: &[u8; 32],
) -> Result<String, ExportError> {
    let output_name = format!("{base_name}.docx");
    let final_path = destination.join(&output_name);
    let temporary_path = destination.join(format!(".{}.tmp", Uuid::now_v7()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    let result = (|| {
        let mut document = Docx::new()
            .add_paragraph(Paragraph::new().add_run(Run::new().add_text(&snapshot.title)));
        match snapshot.layout {
            ExportLayout::QuestionAnswerAlternating => {
                for (index, problem) in problems.iter().enumerate() {
                    document = add_problem_heading(document, index, problem, "题目");
                    document = add_assets(document, problem, "question", blob_root, asset_key)?;
                    document = document
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("答案")));
                    document = add_assets(document, problem, "answer", blob_root, asset_key)?;
                }
            }
            ExportLayout::QuestionsThenAnswers => {
                document =
                    document.add_paragraph(Paragraph::new().add_run(Run::new().add_text("题目")));
                for (index, problem) in problems.iter().enumerate() {
                    document = add_problem_heading(document, index, problem, "题目");
                    document = add_assets(document, problem, "question", blob_root, asset_key)?;
                }
                document =
                    document.add_paragraph(Paragraph::new().add_run(Run::new().add_text("答案")));
                for (index, problem) in problems.iter().enumerate() {
                    document = add_problem_heading(document, index, problem, "答案");
                    document = add_assets(document, problem, "answer", blob_root, asset_key)?;
                }
            }
            ExportLayout::OriginalImageFolder => {
                unreachable!("folder export is handled separately")
            }
        }
        document
            .build()
            .pack(file)
            .map_err(|error| ExportError::Docx(error.to_string()))?;
        OpenOptions::new()
            .write(true)
            .open(&temporary_path)?
            .sync_all()?;
        std::fs::rename(&temporary_path, &final_path)?;
        Ok::<(), ExportError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result?;
    Ok(output_name)
}

fn add_problem_heading(
    document: Docx,
    index: usize,
    problem: &ExportProblem,
    section: &str,
) -> Docx {
    let note = if problem.note.trim().is_empty() {
        String::new()
    } else {
        format!(" · {}", problem.note.trim())
    };
    document.add_paragraph(Paragraph::new().add_run(Run::new().add_text(format!(
        "{}. {} · {}{}",
        index + 1,
        section,
        problem.subject,
        note
    ))))
}

fn add_assets(
    mut document: Docx,
    problem: &ExportProblem,
    role: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
) -> Result<Docx, ExportError> {
    for asset in problem.assets.iter().filter(|asset| asset.role == role) {
        let bytes = read_decrypted_export_asset(blob_root, asset_key, &asset.encrypted_path)?;
        if bytes.len() != asset.byte_length {
            return Err(ExportError::InvalidImage);
        }
        let (png, width, height) = docx_png(&bytes, &asset.media_type)?;
        let scale = (560_f64 / f64::from(width))
            .min(700_f64 / f64::from(height))
            .min(1.0);
        let width_emu = (f64::from(width) * scale * 9_525_f64).round() as u32;
        let height_emu = (f64::from(height) * scale * 9_525_f64).round() as u32;
        let picture = Pic::new(&png).size(width_emu, height_emu);
        document = document
            .add_paragraph(Paragraph::new().add_run(Run::new().add_text("").add_image(picture)));
    }
    Ok(document)
}

fn validate_docx_assets(
    problems: &[ExportProblem],
    blob_root: &Path,
    asset_key: &[u8; 32],
) -> Result<(), ExportError> {
    let mut total_pixels = 0_u64;
    for asset in problems.iter().flat_map(|problem| &problem.assets) {
        let bytes = read_decrypted_export_asset(blob_root, asset_key, &asset.encrypted_path)?;
        if bytes.len() != asset.byte_length {
            return Err(ExportError::InvalidImage);
        }
        let (width, height, _) = docx_dimensions(&bytes, &asset.media_type)?;
        total_pixels = total_pixels
            .checked_add(u64::from(width) * u64::from(height))
            .filter(|total| *total <= MAX_DOCX_TOTAL_PIXELS)
            .ok_or(ExportError::ExportTooLarge)?;
    }
    Ok(())
}

fn docx_png(bytes: &[u8], media_type: &str) -> Result<(Vec<u8>, u32, u32), ExportError> {
    let (width, height, format) = docx_dimensions(bytes, media_type)?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|_| ExportError::InvalidImage)?;
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|_| ExportError::InvalidImage)?;
    Ok((png.into_inner(), width, height))
}

fn docx_dimensions(
    bytes: &[u8],
    media_type: &str,
) -> Result<(u32, u32, image::ImageFormat), ExportError> {
    let format = match media_type {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        _ => return Err(ExportError::InvalidImage),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| ExportError::InvalidImage)?;
    if width == 0
        || height == 0
        || width > MAX_DOCX_IMAGE_DIMENSION
        || height > MAX_DOCX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_DOCX_IMAGE_PIXELS
    {
        return Err(ExportError::InvalidImage);
    }
    Ok((width, height, format))
}

fn media_extension(media_type: &str) -> Result<&'static str, ExportError> {
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        _ => Err(ExportError::InvalidImage),
    }
}

fn safe_output_stem(title: &str) -> String {
    let cleaned = title
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(48)
        .collect::<String>();
    let cleaned = cleaned.trim_matches([' ', '.', '_']);
    if cleaned.is_empty() {
        "mistake-trainer-export".to_owned()
    } else {
        cleaned.to_owned()
    }
}
