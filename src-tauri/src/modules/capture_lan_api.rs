use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use crate::modules::capture_inbox::{
    ApplyCaptureCrop, CaptureBatchState, CaptureCropRecipe, CaptureInboxError, CaptureItemPreview,
    IngestCaptureItem, NormalizedCropRect, RevertCaptureCrop, apply_capture_crop,
    get_capture_batch_detail, get_capture_item_preview, ingest_capture_item, remove_capture_item,
    revert_capture_crop, update_capture_batch,
};

use super::{ServerState, constant_time_eq, current_utc_millis, session_temp_root};

const MOBILE_PAGE: &str = include_str!("../../mobile/capture.html");
const HEIC2ANY_SCRIPT: &str = include_str!("../../mobile/vendor/heic2any.js");
const MAX_ORIGINAL_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;
const UPLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn build_router(state: Arc<ServerState>) -> Router {
    let api = Router::new()
        .route("/session", get(get_session).patch(patch_session))
        .route("/session/finish", post(finish_session))
        .route("/uploads/{client_upload_id}", put(upload_item))
        .route("/items/{item_id}/preview", get(item_preview))
        .route("/items/{item_id}/crop", post(crop_item))
        .route("/items/{item_id}/crop/revert", post(revert_item_crop))
        .route("/items/{item_id}", delete(delete_item))
        .layer(DefaultBodyLimit::max(
            usize::try_from(MAX_ORIGINAL_UPLOAD_BYTES).unwrap_or(usize::MAX),
        ))
        .layer(middleware::map_response(harden_api_response));
    Router::new()
        .route("/mobile/", get(mobile_page))
        .route("/mobile/vendor/heic2any.js", get(heic2any_script))
        .nest("/api/v1", api)
        .with_state(state)
}

async fn harden_api_response(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn mobile_page() -> Response {
    (
        [
            ("content-type", "text/html; charset=utf-8"),
            ("cache-control", "no-store"),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
            (
                "content-security-policy",
                "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; worker-src blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
        ],
        MOBILE_PAGE,
    )
        .into_response()
}

async fn heic2any_script() -> Response {
    (
        [
            ("content-type", "application/javascript; charset=utf-8"),
            ("cache-control", "private, max-age=86400"),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
        ],
        HEIC2ANY_SCRIPT,
    )
        .into_response()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileSessionPayload {
    session_id: String,
    batch_id: String,
    subject: String,
    state: String,
    expires_at_utc_ms: i64,
    received_item_count: u32,
    received_bytes: u64,
    next_source_sequence: u32,
    items: Vec<MobileSessionItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileSessionItem {
    item_id: String,
    source_name: String,
    source_sequence: u32,
    byte_length: u64,
    crop_derivation_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileUploadPayload {
    item_id: String,
    byte_length: u64,
    duplicate: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MobileCropRequest {
    rect: NormalizedCropRect,
    rotation_degrees: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileCropPayload {
    item_id: String,
    source_item_id: String,
    crop_derivation_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MobileSubjectPatch {
    subject: String,
}

async fn get_session(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Result<Json<MobileSessionPayload>, ApiError> {
    authorize(&state, &headers)?;
    session_payload(&state).map(Json)
}

async fn patch_session(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(input): Json<MobileSubjectPatch>,
) -> Result<Json<MobileSessionPayload>, ApiError> {
    authorize(&state, &headers)?;
    if input.subject.chars().count() > 40 {
        return Err(ApiError::bad_request(
            "subject_invalid",
            "科目不能超过 40 个字。",
        ));
    }
    let now = current_utc_millis();
    {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let detail = get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?;
        if detail.batch.state != CaptureBatchState::Collecting {
            return Err(ApiError::conflict("session_finished", "这批采集已经结束。"));
        }
        update_capture_batch(
            &mut connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
            detail.batch.revision,
            &input.subject,
            false,
            now,
        )
        .map_err(ApiError::capture)?;
    }
    state.touch(now);
    (state.context.notifier)(&state.context.batch_id);
    session_payload(&state).map(Json)
}

async fn upload_item(
    State(state): State<Arc<ServerState>>,
    Path(client_upload_id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<MobileUploadPayload>, ApiError> {
    authorize(&state, &headers)?;
    let client_upload_id = Uuid::parse_str(&client_upload_id)
        .map_err(|_| ApiError::bad_request("upload_id_invalid", "上传编号无效。"))?
        .to_string();
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if !matches!(media_type, Some("image/jpeg" | "image/png" | "image/webp")) {
        return Err(ApiError::unsupported_media_type());
    }
    let client_source_sequence = headers
        .get("x-source-sequence")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| ApiError::bad_request("source_sequence_invalid", "图片顺序无效。"))?;
    let source_sequence = state
        .sequence_base
        .checked_add(i64::from(client_source_sequence))
        .ok_or_else(|| ApiError::bad_request("source_sequence_invalid", "图片顺序无效。"))?;
    state.record_source_sequence(client_source_sequence, current_utc_millis());
    let source_name = headers
        .get("x-source-name")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.is_ascii())
        .unwrap_or("mobile-image")
        .to_owned();
    let _permit = state
        .upload_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal())?;
    let temp_root = session_temp_root(&state);
    tokio::fs::create_dir_all(&temp_root)
        .await
        .map_err(|_| ApiError::internal())?;
    let temp_path = temp_root.join(format!("{client_upload_id}.upload"));
    let mut output = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .await
        .map_err(|_| ApiError::conflict("upload_in_progress", "这张图片正在上传，请稍候。"))?;
    let mut stream = body.into_data_stream();
    let mut received = 0_u64;
    let stream_result = async {
        loop {
            let next = tokio::time::timeout(UPLOAD_STALL_TIMEOUT, stream.next())
                .await
                .map_err(|_| ApiError::request_timeout())?;
            let Some(chunk) = next else {
                break;
            };
            let chunk =
                chunk.map_err(|_| ApiError::bad_request("upload_broken", "上传内容不完整。"))?;
            received = received
                .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
                .filter(|total| *total <= MAX_ORIGINAL_UPLOAD_BYTES)
                .ok_or_else(ApiError::too_large)?;
            output
                .write_all(&chunk)
                .await
                .map_err(|_| ApiError::internal())?;
            state.touch(current_utc_millis());
        }
        output.flush().await.map_err(|_| ApiError::internal())?;
        output.sync_all().await.map_err(|_| ApiError::internal())?;
        Ok::<(), ApiError>(())
    }
    .await;
    drop(output);
    if let Err(error) = stream_result {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(error);
    }
    let bytes = tokio::fs::read(&temp_path)
        .await
        .map_err(|_| ApiError::internal())?;
    let _ = tokio::fs::remove_file(&temp_path).await;
    let now = current_utc_millis();
    let (item, duplicate) = {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let duplicate: bool = connection
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM capture_items WHERE batch_id = ?1 AND client_upload_id = ?2
                 )",
                rusqlite::params![state.context.batch_id, client_upload_id],
                |row| row.get(0),
            )
            .map_err(|_| ApiError::internal())?;
        let item = ingest_capture_item(
            &mut connection,
            &state.context.blob_root,
            &state.context.asset_key,
            IngestCaptureItem {
                account_id: state.context.account_id.clone(),
                profile_id: state.context.profile_id.clone(),
                batch_id: state.context.batch_id.clone(),
                client_upload_id,
                source_name,
                source_sequence: Some(source_sequence),
                bytes,
                now_utc_ms: now,
            },
        )
        .map_err(ApiError::capture)?;
        (item, duplicate)
    };
    if !duplicate {
        state.record_upload(item.byte_length as u64, now);
        (state.context.notifier)(&state.context.batch_id);
    } else {
        state.touch(now);
    }
    Ok(Json(MobileUploadPayload {
        item_id: item.id,
        byte_length: item.byte_length as u64,
        duplicate,
    }))
}

async fn delete_item(
    State(state): State<Arc<ServerState>>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers)?;
    let now = current_utc_millis();
    let byte_length = {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let detail = get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?;
        let item = detail
            .items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| ApiError::not_found("item_missing", "这张图片已经不存在。"))?;
        let byte_length = item.byte_length as u64;
        remove_capture_item(
            &mut connection,
            &state.context.blob_root,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
            detail.batch.revision,
            &item_id,
            now,
        )
        .map_err(ApiError::capture)?;
        byte_length
    };
    state.record_delete(byte_length, now);
    (state.context.notifier)(&state.context.batch_id);
    Ok(StatusCode::NO_CONTENT)
}

async fn item_preview(
    State(state): State<Arc<ServerState>>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CaptureItemPreview>, ApiError> {
    authorize(&state, &headers)?;
    let preview = {
        let connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        get_capture_item_preview(
            &connection,
            &state.context.blob_root,
            &state.context.asset_key,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
            &item_id,
        )
        .map_err(ApiError::capture)?
    };
    state.touch(current_utc_millis());
    Ok(Json(preview))
}

async fn crop_item(
    State(state): State<Arc<ServerState>>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<MobileCropRequest>,
) -> Result<Json<MobileCropPayload>, ApiError> {
    authorize(&state, &headers)?;
    let now = current_utc_millis();
    let report = {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let detail = get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?;
        apply_capture_crop(
            &mut connection,
            &state.context.blob_root,
            &state.context.asset_key,
            ApplyCaptureCrop {
                account_id: state.context.account_id.clone(),
                profile_id: state.context.profile_id.clone(),
                batch_id: state.context.batch_id.clone(),
                expected_revision: detail.batch.revision,
                item_id: item_id.clone(),
                recipes: vec![CaptureCropRecipe {
                    rect: input.rect,
                    perspective_quad: None,
                    rotation_degrees: input.rotation_degrees,
                    output_media_type: "image/png".to_owned(),
                    max_edge: 4_096,
                    jpeg_quality: 90,
                }],
                allow_collecting: true,
                now_utc_ms: now,
            },
        )
        .map_err(ApiError::capture)?
    };
    let derived_item_id = report
        .derived_item_ids
        .first()
        .cloned()
        .ok_or_else(ApiError::internal)?;
    let derivation_id = report
        .derivation_ids
        .first()
        .cloned()
        .ok_or_else(ApiError::internal)?;
    state.touch(now);
    (state.context.notifier)(&state.context.batch_id);
    Ok(Json(MobileCropPayload {
        item_id: derived_item_id,
        source_item_id: report.source_item_id,
        crop_derivation_id: derivation_id,
    }))
}

async fn revert_item_crop(
    State(state): State<Arc<ServerState>>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers)?;
    let now = current_utc_millis();
    {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let detail = get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?;
        let derivation_id = detail
            .items
            .iter()
            .find(|item| item.id == item_id)
            .and_then(|item| item.crop_derivation_id.clone())
            .ok_or_else(|| {
                ApiError::bad_request("crop_not_revertible", "这张图片没有可恢复的裁剪原图。")
            })?;
        revert_capture_crop(
            &mut connection,
            &state.context.blob_root,
            RevertCaptureCrop {
                account_id: state.context.account_id.clone(),
                profile_id: state.context.profile_id.clone(),
                batch_id: state.context.batch_id.clone(),
                expected_revision: detail.batch.revision,
                derivation_id,
                allow_collecting: true,
                now_utc_ms: now,
            },
        )
        .map_err(ApiError::capture)?;
    }
    state.touch(now);
    (state.context.notifier)(&state.context.batch_id);
    Ok(StatusCode::NO_CONTENT)
}

async fn finish_session(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers)?;
    let now = current_utc_millis();
    {
        let mut connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        let detail = get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?;
        if detail.batch.state == CaptureBatchState::Collecting {
            update_capture_batch(
                &mut connection,
                &state.context.account_id,
                &state.context.profile_id,
                &state.context.batch_id,
                detail.batch.revision,
                &detail.batch.subject,
                true,
                now,
            )
            .map_err(ApiError::capture)?;
        }
    }
    state.touch(now);
    (state.context.notifier)(&state.context.batch_id);
    let _ = state.shutdown.send(true);
    Ok(StatusCode::NO_CONTENT)
}

fn session_payload(state: &ServerState) -> Result<MobileSessionPayload, ApiError> {
    let detail = {
        let connection = state
            .context
            .connection
            .lock()
            .map_err(|_| ApiError::internal())?;
        get_capture_batch_detail(
            &connection,
            &state.context.account_id,
            &state.context.profile_id,
            &state.context.batch_id,
        )
        .map_err(ApiError::capture)?
    };
    let activity = state.activity_snapshot();
    Ok(MobileSessionPayload {
        session_id: state.session_id.clone(),
        batch_id: state.context.batch_id.clone(),
        subject: detail.batch.subject,
        state: match detail.batch.state {
            CaptureBatchState::Collecting => "collecting",
            CaptureBatchState::Organizing => "organizing",
            CaptureBatchState::Completed => "completed",
        }
        .to_owned(),
        expires_at_utc_ms: state.expires_at(&activity),
        received_item_count: activity.received_item_count,
        received_bytes: activity.received_bytes,
        next_source_sequence: activity.next_source_sequence,
        items: detail
            .items
            .into_iter()
            .map(|item| MobileSessionItem {
                item_id: item.id,
                source_name: item.source_name,
                source_sequence: item.source_sequence,
                byte_length: item.byte_length.max(0.0) as u64,
                crop_derivation_id: item.crop_derivation_id,
            })
            .collect(),
    })
}

fn authorize(state: &ServerState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.is_expired(current_utc_millis()) {
        let _ = state.shutdown.send(true);
        return Err(ApiError::gone(
            "session_expired",
            "采集会话已过期，请在电脑上重新开启。",
        ));
    }
    let host_matches = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_host);
    let origin_matches = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value == state.public_origin);
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    let supplied_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    if !host_matches || !origin_matches || !constant_time_eq(&supplied_hash, &state.token_hash) {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: &'static str,
    message: &'static str,
    retryable: bool,
}

struct ApiError {
    status: StatusCode,
    body: ApiErrorBody,
}

impl ApiError {
    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "采集令牌无效。",
            false,
        )
    }

    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "capture_failed",
            "电脑没有完成这次操作，请稍后重试。",
            true,
        )
    }

    fn too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "upload_too_large",
            "单张原图不能超过 50 MB。",
            false,
        )
    }

    fn request_timeout() -> Self {
        Self::new(
            StatusCode::REQUEST_TIMEOUT,
            "upload_stalled",
            "上传等待超时，请检查手机网络后重试。",
            true,
        )
    }

    fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "media_type_unsupported",
            "手机只能上传 JPEG、PNG 或 WebP 图片。",
            false,
        )
    }

    fn bad_request(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message, false)
    }

    fn conflict(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, code, message, true)
    }

    fn not_found(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, false)
    }

    fn gone(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::GONE, code, message, false)
    }

    fn capture(error: CaptureInboxError) -> Self {
        match error {
            CaptureInboxError::InvalidImage => Self::bad_request(
                "image_invalid",
                "图片损坏、过大或格式不支持；请使用 JPEG、PNG 或 WebP。",
            ),
            CaptureInboxError::CapacityReached => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "batch_full",
                "这一批已达到 150 张或 1 GB 上限。",
                false,
            ),
            CaptureInboxError::InvalidState => {
                Self::conflict("session_finished", "这批采集已经结束。")
            }
            CaptureInboxError::BatchNotFound => {
                Self::not_found("batch_missing", "采集批次已经不存在。")
            }
            CaptureInboxError::ItemNotFound => {
                Self::not_found("item_missing", "这张图片已经不存在。")
            }
            CaptureInboxError::RevisionConflict => {
                Self::conflict("batch_changed", "电脑端刚刚修改了这批内容，请重试。")
            }
            CaptureInboxError::InvalidCrop => {
                Self::bad_request("crop_invalid", "裁剪范围无效，请重新调整。")
            }
            CaptureInboxError::CropNotRevertible => {
                Self::bad_request("crop_not_revertible", "这张图片的裁剪原图已经不能恢复。")
            }
            _ => Self::internal(),
        }
    }

    const fn new(
        status: StatusCode,
        code: &'static str,
        message: &'static str,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                code,
                message,
                retryable,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[cfg(test)]
#[path = "capture_lan_api_tests.rs"]
mod tests;
