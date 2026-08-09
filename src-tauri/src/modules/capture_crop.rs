use std::{
    collections::{BTreeSet, HashMap},
    io::Cursor,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::infrastructure::assets::{decrypt_asset, encrypt_asset, plaintext_sha256};

use super::capture_inbox_transaction_support::{
    delete_asset_row_if_orphan, invalidate_active_pairs_for_item, repack_link_positions,
    touch_batch,
};
use super::{
    CaptureBatchDetail, CaptureBatchState, CaptureBatchSummary, CaptureInboxError,
    MAX_CAPTURE_BATCH_BYTES, MAX_CAPTURE_BATCH_ITEMS, get_capture_batch_detail,
    image_format_for_media_type, query_batch, read_encrypted_blob, remove_encrypted_blob,
    sanitize_source_name,
};

const CAPTURE_PREVIEW_MAX_DIMENSION: u32 = 960;
const CROP_SOURCE_PREVIEW_MAX_DIMENSION: u32 = 2_400;
const MAX_CROP_REGIONS: usize = 10;
const CROP_SOURCE_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItemPreview {
    pub item_id: String,
    pub media_type: String,
    pub data_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCropRecipe {
    pub rect: NormalizedCropRect,
    pub rotation_degrees: u16,
    pub output_media_type: String,
    pub max_edge: u32,
    pub jpeg_quality: u8,
}

#[derive(Clone, Debug)]
pub struct ApplyCaptureCrop {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub recipes: Vec<CaptureCropRecipe>,
    pub allow_collecting: bool,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct RevertCaptureCrop {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub derivation_id: String,
    pub allow_collecting: bool,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCropApplyReport {
    pub detail: CaptureBatchDetail,
    pub operation_id: String,
    pub source_item_id: String,
    pub derived_item_ids: Vec<String>,
    pub derivation_ids: Vec<String>,
}

pub fn get_capture_item_preview(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemPreview, CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let (media_type, encrypted_path) = connection
        .query_row(
            "SELECT a.media_type, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let encrypted = read_encrypted_blob(blob_root, &encrypted_path)?;
    let plaintext = decrypt_asset(&encrypted, key).map_err(|_| CaptureInboxError::Crypto)?;
    let format = image_format_for_media_type(&media_type)?;
    let image = image::load_from_memory_with_format(&plaintext, format)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    let thumbnail = image.thumbnail(CAPTURE_PREVIEW_MAX_DIMENSION, CAPTURE_PREVIEW_MAX_DIMENSION);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    Ok(CaptureItemPreview {
        item_id: item_id.to_owned(),
        media_type: "image/png".to_owned(),
        data_url: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(output.into_inner())
        ),
    })
}

pub fn get_capture_crop_source_preview(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemPreview, CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let (media_type, encrypted_path) = connection
        .query_row(
            "SELECT a.media_type, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let plaintext = decrypt_asset(&read_encrypted_blob(blob_root, &encrypted_path)?, key)
        .map_err(|_| CaptureInboxError::Crypto)?;
    let image =
        image::load_from_memory_with_format(&plaintext, image_format_for_media_type(&media_type)?)
            .map_err(|_| CaptureInboxError::InvalidImage)?;
    let preview = image.thumbnail(
        CROP_SOURCE_PREVIEW_MAX_DIMENSION,
        CROP_SOURCE_PREVIEW_MAX_DIMENSION,
    );
    let mut output = Cursor::new(Vec::new());
    preview
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    Ok(CaptureItemPreview {
        item_id: item_id.to_owned(),
        media_type: "image/png".to_owned(),
        data_url: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(output.into_inner())
        ),
    })
}

struct CropSource {
    asset_id: String,
    media_type: String,
    encrypted_path: String,
    source_name: String,
    source_sequence: i64,
    staged_role: String,
}

#[derive(Debug)]
pub(crate) struct EncodedCrop {
    pub(crate) bytes: Vec<u8>,
    pub(crate) media_type: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) recipe_json: String,
}

struct StagedCropAsset {
    id: String,
    hash: String,
    media_type: String,
    byte_length: i64,
    relative: String,
    staged_path: PathBuf,
    final_path: PathBuf,
}

fn validate_crop_recipe(recipe: &CaptureCropRecipe) -> Result<(), CaptureInboxError> {
    let rect = &recipe.rect;
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f64::is_finite)
        || rect.x < 0.0
        || rect.y < 0.0
        || rect.width <= 0.0
        || rect.height <= 0.0
        || rect.x + rect.width > 1.0 + f64::EPSILON
        || rect.y + rect.height > 1.0 + f64::EPSILON
        || !matches!(recipe.rotation_degrees, 0 | 90 | 180 | 270)
        || !matches!(
            recipe.output_media_type.as_str(),
            "image/png" | "image/jpeg"
        )
        || !(320..=12_000).contains(&recipe.max_edge)
        || !(70..=100).contains(&recipe.jpeg_quality)
    {
        return Err(CaptureInboxError::InvalidCrop);
    }
    Ok(())
}

pub(crate) fn encode_crop(
    source: &image::DynamicImage,
    recipe: &CaptureCropRecipe,
) -> Result<EncodedCrop, CaptureInboxError> {
    validate_crop_recipe(recipe)?;
    let rotated = match recipe.rotation_degrees {
        90 => source.rotate90(),
        180 => source.rotate180(),
        270 => source.rotate270(),
        _ => source.clone(),
    };
    let image_width = rotated.width();
    let image_height = rotated.height();
    let left = (recipe.rect.x * f64::from(image_width)).floor() as u32;
    let top = (recipe.rect.y * f64::from(image_height)).floor() as u32;
    let right = ((recipe.rect.x + recipe.rect.width) * f64::from(image_width)).ceil() as u32;
    let bottom = ((recipe.rect.y + recipe.rect.height) * f64::from(image_height)).ceil() as u32;
    let right = right.min(image_width);
    let bottom = bottom.min(image_height);
    if left >= right || top >= bottom {
        return Err(CaptureInboxError::InvalidCrop);
    }
    let mut cropped = rotated.crop_imm(left, top, right - left, bottom - top);
    if cropped.width() > recipe.max_edge || cropped.height() > recipe.max_edge {
        cropped = cropped.thumbnail(recipe.max_edge, recipe.max_edge);
    }
    let mut output = Cursor::new(Vec::new());
    match recipe.output_media_type.as_str() {
        "image/png" => cropped
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|_| CaptureInboxError::InvalidImage)?,
        "image/jpeg" => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, recipe.jpeg_quality)
                .encode_image(&cropped)
                .map_err(|_| CaptureInboxError::InvalidImage)?
        }
        _ => return Err(CaptureInboxError::InvalidCrop),
    }
    let bytes = output.into_inner();
    if bytes.len() > 25 * 1024 * 1024 {
        return Err(CaptureInboxError::InvalidImage);
    }
    Ok(EncodedCrop {
        width: cropped.width(),
        height: cropped.height(),
        recipe_json: serde_json::to_string(recipe)?,
        media_type: recipe.output_media_type.clone(),
        bytes,
    })
}

pub fn apply_capture_crop(
    connection: &mut Connection,
    blob_root: &Path,
    asset_key: &[u8; 32],
    input: ApplyCaptureCrop,
) -> Result<CaptureCropApplyReport, CaptureInboxError> {
    if input.recipes.is_empty() || input.recipes.len() > MAX_CROP_REGIONS {
        return Err(CaptureInboxError::InvalidCrop);
    }
    for recipe in &input.recipes {
        validate_crop_recipe(recipe)?;
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_crop_revision(&batch, input.expected_revision, input.allow_collecting)?;
    let source = connection
        .query_row(
            "SELECT i.asset_id, a.media_type, a.encrypted_path, i.source_name,
                    i.source_sequence, i.staged_role
             FROM capture_items i JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![input.item_id, input.batch_id, input.account_id],
            |row| {
                Ok(CropSource {
                    asset_id: row.get(0)?,
                    media_type: row.get(1)?,
                    encrypted_path: row.get(2)?,
                    source_name: row.get(3)?,
                    source_sequence: row.get(4)?,
                    staged_role: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let plaintext = decrypt_asset(
        &read_encrypted_blob(blob_root, &source.encrypted_path)?,
        asset_key,
    )
    .map_err(|_| CaptureInboxError::Crypto)?;
    let source_image = image::load_from_memory_with_format(
        &plaintext,
        image_format_for_media_type(&source.media_type)?,
    )
    .map_err(|_| CaptureInboxError::InvalidImage)?;
    let encoded = input
        .recipes
        .iter()
        .map(|recipe| encode_crop(&source_image, recipe))
        .collect::<Result<Vec<_>, _>>()?;
    let (active_count, stored_bytes): (i64, i64) = connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL),
           (SELECT COALESCE(SUM(a.byte_length), 0) FROM capture_items i JOIN assets a ON a.id = i.asset_id WHERE i.batch_id = ?1)",
        [input.batch_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let resulting_count = active_count - 1 + i64::try_from(encoded.len()).unwrap_or(i64::MAX);
    let added_bytes = encoded.iter().fold(0_i64, |total, crop| {
        total.saturating_add(i64::try_from(crop.bytes.len()).unwrap_or(i64::MAX))
    });
    if resulting_count > MAX_CAPTURE_BATCH_ITEMS
        || stored_bytes.saturating_add(added_bytes) > MAX_CAPTURE_BATCH_BYTES
    {
        return Err(CaptureInboxError::CapacityReached);
    }

    let staging_root = blob_root.join(".staging");
    std::fs::create_dir_all(&staging_root)?;
    let mut asset_by_hash = HashMap::<String, String>::new();
    let mut staged_assets = Vec::<StagedCropAsset>::new();
    let mut derived_asset_ids = Vec::with_capacity(encoded.len());
    let stage_result = (|| -> Result<(), CaptureInboxError> {
        for crop in &encoded {
            let hash = plaintext_sha256(&crop.bytes);
            if let Some(asset_id) = asset_by_hash.get(&hash) {
                derived_asset_ids.push(asset_id.clone());
                continue;
            }
            if let Some(asset_id) = connection
                .query_row(
                    "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
                    params![input.account_id, hash],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                asset_by_hash.insert(hash, asset_id.clone());
                derived_asset_ids.push(asset_id);
                continue;
            }
            let asset_id = Uuid::now_v7().to_string();
            let relative_path = PathBuf::from("blobs")
                .join(&asset_id[..2])
                .join(format!("{asset_id}.mtb"));
            let staged_path = staging_root.join(format!("{asset_id}.crop.tmp"));
            let final_path = blob_root.join(&relative_path);
            let encrypted =
                encrypt_asset(&crop.bytes, asset_key).map_err(|_| CaptureInboxError::Crypto)?;
            if let Err(error) = std::fs::write(&staged_path, encrypted) {
                let _ = std::fs::remove_file(&staged_path);
                return Err(error.into());
            }
            asset_by_hash.insert(hash.clone(), asset_id.clone());
            derived_asset_ids.push(asset_id.clone());
            staged_assets.push(StagedCropAsset {
                id: asset_id,
                hash,
                media_type: crop.media_type.clone(),
                byte_length: i64::try_from(crop.bytes.len()).unwrap_or(i64::MAX),
                relative: relative_path.to_string_lossy().replace('\\', "/"),
                staged_path,
                final_path,
            });
        }
        Ok(())
    })();
    if let Err(error) = stage_result {
        for asset in &staged_assets {
            let _ = std::fs::remove_file(&asset.staged_path);
        }
        return Err(error);
    }

    let operation_id = Uuid::now_v7().to_string();
    let derived_item_ids = (0..encoded.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let derivation_ids = (0..encoded.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let transaction = connection.transaction()?;
    let persist_result = (|| -> Result<(), CaptureInboxError> {
        invalidate_active_pairs_for_item(&transaction, &input.item_id, input.now_utc_ms)?;
        for asset in &staged_assets {
            transaction.execute(
                "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![asset.id, input.account_id, asset.hash, asset.relative, asset.byte_length, asset.media_type, input.now_utc_ms],
            )?;
            if let Some(parent) = asset.final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&asset.staged_path, &asset.final_path)?;
        }
        let source_link = transaction
            .query_row(
                "SELECT draft_id, role, position FROM capture_draft_items WHERE item_id = ?1",
                [input.item_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        transaction.execute(
            "DELETE FROM capture_draft_items WHERE item_id = ?1",
            [input.item_id.as_str()],
        )?;
        let sequence_shift = i64::try_from(encoded.len()).unwrap_or(i64::MAX);
        let sequence_offset: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(source_sequence), 0) + 1000 FROM capture_items WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE capture_items SET source_sequence = source_sequence + ?1
             WHERE batch_id = ?2 AND source_sequence >= ?3",
            params![sequence_offset, input.batch_id, source.source_sequence],
        )?;
        transaction.execute(
            "UPDATE capture_items SET source_sequence = source_sequence - ?1 + ?2
             WHERE batch_id = ?3 AND source_sequence >= ?4",
            params![
                sequence_offset,
                sequence_shift,
                input.batch_id,
                source.source_sequence + sequence_offset
            ],
        )?;
        let extension = if encoded[0].media_type == "image/jpeg" {
            "jpg"
        } else {
            "png"
        };
        let base_name = Path::new(&source.source_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("image");
        for (position, crop) in encoded.iter().enumerate() {
            let item_id = &derived_item_ids[position];
            let derivation_id = &derivation_ids[position];
            let source_name = sanitize_source_name(&format!(
                "{base_name}-crop-{}.{}",
                position + 1,
                if crop.media_type == "image/jpeg" {
                    "jpg"
                } else {
                    extension
                }
            ));
            transaction.execute(
                "INSERT INTO capture_items(
                   id, batch_id, asset_id, client_upload_id, source_name, source_sequence,
                   width, height, created_at_utc_ms, staged_role
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    item_id,
                    input.batch_id,
                    derived_asset_ids[position],
                    format!("crop:{operation_id}:{position}"),
                    source_name,
                    source.source_sequence + i64::try_from(position).unwrap_or(i64::MAX),
                    crop.width,
                    crop.height,
                    input.now_utc_ms,
                    source.staged_role
                ],
            )?;
            transaction.execute(
                "INSERT INTO asset_derivations(
                   id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                   source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                   engine, engine_version, confidence, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'crop', ?10, 'mistake-trainer-crop', '1', NULL, ?11)",
                params![derivation_id, operation_id, input.account_id, input.batch_id, source.asset_id, derived_asset_ids[position], input.item_id, item_id, i64::try_from(position).unwrap_or(i64::MAX), crop.recipe_json, input.now_utc_ms],
            )?;
        }
        if let Some((draft_id, role, position)) = source_link {
            transaction.execute(
                "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
                 VALUES(?1, ?2, ?3, ?4)",
                params![draft_id, derived_item_ids[0], role, position],
            )?;
        }
        transaction.execute(
            "UPDATE capture_items SET superseded_by_derivation_id = ?1
             WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NULL",
            params![derivation_ids[0], input.item_id, input.batch_id],
        )?;
        transaction.execute(
            "INSERT INTO capture_source_retention(batch_id, source_asset_id, retain_until_utc_ms, reason, created_at_utc_ms)
             VALUES(?1, ?2, ?3, 'crop_recovery', ?4)
             ON CONFLICT(batch_id, source_asset_id) DO UPDATE SET retain_until_utc_ms = excluded.retain_until_utc_ms",
            params![input.batch_id, source.asset_id, input.now_utc_ms.saturating_add(CROP_SOURCE_RETENTION_MS), input.now_utc_ms],
        )?;
        touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = persist_result {
        for asset in &staged_assets {
            let _ = std::fs::remove_file(&asset.staged_path);
            let _ = std::fs::remove_file(&asset.final_path);
        }
        return Err(error);
    }
    let detail = get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    Ok(CaptureCropApplyReport {
        detail,
        operation_id,
        source_item_id: input.item_id,
        derived_item_ids,
        derivation_ids,
    })
}

pub fn revert_capture_crop(
    connection: &mut Connection,
    blob_root: &Path,
    input: RevertCaptureCrop,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_crop_revision(&batch, input.expected_revision, input.allow_collecting)?;
    let (operation_id, source_item_id, source_asset_id): (String, String, String) = connection
        .query_row(
            "SELECT operation_id, source_capture_item_id, source_asset_id
             FROM asset_derivations
             WHERE id = ?1 AND batch_id = ?2 AND account_id = ?3",
            params![input.derivation_id, input.batch_id, input.account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::CropNotRevertible)?;
    let transaction = connection.transaction()?;
    let derived = {
        let mut statement = transaction.prepare(
            "SELECT d.derived_capture_item_id, d.derived_asset_id, a.encrypted_path, d.position,
                    i.source_sequence
             FROM asset_derivations d
             JOIN assets a ON a.id = d.derived_asset_id
             JOIN capture_items i ON i.id = d.derived_capture_item_id
             WHERE d.operation_id = ?1 AND d.batch_id = ?2
             ORDER BY d.position",
        )?;
        statement
            .query_map(params![operation_id, input.batch_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if derived.is_empty() {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    for (item_id, _, _, _, _) in &derived {
        invalidate_active_pairs_for_item(&transaction, item_id, input.now_utc_ms)?;
    }
    let existing_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_items
         WHERE batch_id = ?1 AND id IN (
           SELECT derived_capture_item_id FROM asset_derivations WHERE operation_id = ?2
         )",
        params![input.batch_id, operation_id],
        |row| row.get(0),
    )?;
    if existing_count != i64::try_from(derived.len()).unwrap_or(i64::MAX) {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    let first_link = transaction
        .query_row(
            "SELECT di.draft_id, di.role, di.position
             FROM asset_derivations d
             JOIN capture_draft_items di ON di.item_id = d.derived_capture_item_id
             WHERE d.operation_id = ?1 AND d.position = 0",
            [operation_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let affected_links = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT di.draft_id, di.role
             FROM asset_derivations d JOIN capture_draft_items di ON di.item_id = d.derived_capture_item_id
             WHERE d.operation_id = ?1",
        )?;
        statement
            .query_map([operation_id.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let first_sequence = derived
        .iter()
        .map(|entry| entry.4)
        .min()
        .ok_or(CaptureInboxError::CropNotRevertible)?;
    let hidden_sequence: i64 = transaction.query_row(
        "SELECT source_sequence FROM capture_items WHERE id = ?1 AND batch_id = ?2",
        params![source_item_id, input.batch_id],
        |row| row.get(0),
    )?;
    for (item_id, _, _, _, _) in &derived {
        transaction.execute("DELETE FROM capture_items WHERE id = ?1", [item_id])?;
    }
    transaction.execute(
        "DELETE FROM asset_derivations WHERE operation_id = ?1 AND batch_id = ?2",
        params![operation_id, input.batch_id],
    )?;
    transaction.execute(
        "DELETE FROM capture_source_retention WHERE batch_id = ?1 AND source_asset_id = ?2",
        params![input.batch_id, source_asset_id],
    )?;
    let sequence_offset: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(source_sequence), 0) + 1000 FROM capture_items WHERE batch_id = ?1",
        [input.batch_id.as_str()],
        |row| row.get(0),
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence + ?1 WHERE id = ?2 AND batch_id = ?3",
        params![sequence_offset, source_item_id, input.batch_id],
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence + ?1
         WHERE batch_id = ?2 AND superseded_by_derivation_id IS NULL AND source_sequence > ?3",
        params![sequence_offset, input.batch_id, hidden_sequence],
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence - ?1 - ?2
         WHERE batch_id = ?3 AND superseded_by_derivation_id IS NULL AND source_sequence > ?4",
        params![
            sequence_offset,
            i64::try_from(derived.len()).unwrap_or(i64::MAX),
            input.batch_id,
            hidden_sequence + sequence_offset
        ],
    )?;
    let restored = transaction.execute(
        "UPDATE capture_items SET superseded_by_derivation_id = NULL, source_sequence = ?1
         WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NOT NULL",
        params![first_sequence, source_item_id, input.batch_id],
    )?;
    if restored != 1 {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    if let Some((draft_id, role, position)) = first_link {
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![draft_id, source_item_id, role, position],
        )?;
    }
    for (draft_id, role) in affected_links {
        repack_link_positions(&transaction, &draft_id, &role)?;
    }
    let mut orphan_paths = Vec::new();
    let mut seen_assets = BTreeSet::new();
    for (_, asset_id, encrypted_path, _, _) in &derived {
        if seen_assets.insert(asset_id.clone())
            && delete_asset_row_if_orphan(&transaction, asset_id)?
        {
            orphan_paths.push(encrypted_path.clone());
        }
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    for path in orphan_paths {
        let _ = remove_encrypted_blob(blob_root, &path);
    }
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

fn ensure_crop_revision(
    batch: &CaptureBatchSummary,
    expected_revision: u32,
    allow_collecting: bool,
) -> Result<(), CaptureInboxError> {
    let state_allowed = batch.state == CaptureBatchState::Organizing
        || (allow_collecting && batch.state == CaptureBatchState::Collecting);
    if !state_allowed {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    Ok(())
}
