use std::{
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
    thread,
    time::Duration,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt as _;
use qrcode::{QrCode, render::svg};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt as _,
    sync::{Semaphore, watch},
};
use uuid::Uuid;

use crate::modules::capture_inbox::{
    CaptureBatchState, CaptureInboxError, CaptureItemPreview, IngestCaptureItem,
    get_capture_batch_detail, get_capture_item_preview, ingest_capture_item, remove_capture_item,
    update_capture_batch,
};

const MOBILE_PAGE: &str = include_str!("../../mobile/capture.html");
const HEIC2ANY_SCRIPT: &str = include_str!("../../mobile/vendor/heic2any.js");
const MAX_ORIGINAL_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;
const IDLE_TIMEOUT_MS: i64 = 30 * 60 * 1000;
const ABSOLUTE_TIMEOUT_MS: i64 = 2 * 60 * 60 * 1000;
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);
const UPLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(30);

pub type BatchChangeNotifier = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLanAddress {
    pub label: String,
    pub address: String,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLanSession {
    pub session_id: String,
    pub batch_id: String,
    pub qr_svg_data_url: String,
    pub selected_address: String,
    pub expires_at_utc_ms: f64,
    pub received_item_count: u32,
    pub received_bytes: f64,
}

#[derive(Clone)]
pub struct CaptureLanContext {
    pub connection: Arc<Mutex<Connection>>,
    pub blob_root: PathBuf,
    pub asset_key: [u8; 32],
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub notifier: BatchChangeNotifier,
}

#[derive(Debug, Error)]
pub enum CaptureLanError {
    #[error("a phone capture session is already active")]
    AlreadyActive,
    #[error("no private IPv4 address is available")]
    NoPrivateAddress,
    #[error("a network address must be selected")]
    AddressRequired,
    #[error("the selected network address is unavailable")]
    InvalidAddress,
    #[error("the capture batch is unavailable")]
    BatchUnavailable,
    #[error("the capture batch is not collecting")]
    InvalidBatchState,
    #[error("the local capture server could not start")]
    Server(#[from] std::io::Error),
    #[error("the QR code could not be created")]
    Qr,
    #[error("the capture session manager is unavailable")]
    Unavailable,
}

struct ActiveSession {
    session_id: String,
    batch_id: String,
    selected_address: String,
    qr_svg_data_url: String,
    state: Arc<ServerState>,
    shutdown: watch::Sender<bool>,
}

#[derive(Default)]
pub struct CaptureLanManager {
    active: Arc<Mutex<Option<ActiveSession>>>,
}

impl std::fmt::Debug for CaptureLanManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CaptureLanManager")
            .field("active", &"<redacted in-memory session>")
            .finish()
    }
}

impl Drop for CaptureLanManager {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(active) = active.take() {
                let _ = active.shutdown.send(true);
            }
        }
    }
}

impl CaptureLanManager {
    pub fn addresses(&self) -> Result<Vec<CaptureLanAddress>, CaptureLanError> {
        private_ipv4_addresses()
    }

    pub fn start(
        &self,
        context: CaptureLanContext,
        selected_address: Option<&str>,
        now_utc_ms: i64,
    ) -> Result<CaptureLanSession, CaptureLanError> {
        self.stop_expired(now_utc_ms)?;
        if self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?
            .is_some()
        {
            return Err(CaptureLanError::AlreadyActive);
        }

        let sequence_base = collecting_batch_next_sequence(&context)?;
        let addresses = private_ipv4_addresses()?;
        let selected = select_address(&addresses, selected_address)?;
        let selected_ip = selected
            .address
            .parse::<Ipv4Addr>()
            .map_err(|_| CaptureLanError::InvalidAddress)?;
        let listener = TcpListener::bind(SocketAddrV4::new(selected_ip, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        let session_id = Uuid::now_v7().to_string();
        let mut raw_token = [0_u8; 32];
        getrandom::fill(&mut raw_token).map_err(|_| CaptureLanError::Unavailable)?;
        let token = raw_token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let token_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let public_origin = format!("http://{}:{port}", selected.address);
        let mobile_url = format!("{public_origin}/mobile/#{token}");
        let qr_svg = QrCode::new(mobile_url.as_bytes())
            .map_err(|_| CaptureLanError::Qr)?
            .render::<svg::Color>()
            .min_dimensions(320, 320)
            .build();
        let qr_svg_data_url = format!(
            "data:image/svg+xml;base64,{}",
            STANDARD.encode(qr_svg.as_bytes())
        );
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let state = Arc::new(ServerState {
            session_id: session_id.clone(),
            context,
            public_origin,
            expected_host: format!("{}:{port}", selected.address),
            token_hash,
            sequence_base,
            started_at_utc_ms: now_utc_ms,
            activity: Mutex::new(SessionActivity {
                last_activity_utc_ms: now_utc_ms,
                received_item_count: 0,
                received_bytes: 0,
                next_source_sequence: 0,
            }),
            upload_slots: Arc::new(Semaphore::new(2)),
            shutdown: shutdown.clone(),
        });

        {
            let mut active = self
                .active
                .lock()
                .map_err(|_| CaptureLanError::Unavailable)?;
            if active.is_some() {
                return Err(CaptureLanError::AlreadyActive);
            }
            *active = Some(ActiveSession {
                session_id: session_id.clone(),
                batch_id: state.context.batch_id.clone(),
                selected_address: selected.address,
                qr_svg_data_url,
                state: Arc::clone(&state),
                shutdown,
            });
        }

        let weak_active = Arc::downgrade(&self.active);
        let thread_session_id = session_id.clone();
        let server_state = Arc::clone(&state);
        let handle = thread::Builder::new()
            .name("capture-lan-server".to_owned())
            .spawn(move || {
                runtime.block_on(run_server(
                    listener,
                    server_state,
                    shutdown_receiver,
                    weak_active,
                    thread_session_id,
                ));
            });
        match handle {
            Ok(handle) => drop(handle),
            Err(error) => {
                let _ = self.stop();
                return Err(CaptureLanError::Server(error));
            }
        }

        self.status(now_utc_ms)?.ok_or(CaptureLanError::Unavailable)
    }

    pub fn status(&self, now_utc_ms: i64) -> Result<Option<CaptureLanSession>, CaptureLanError> {
        self.stop_expired(now_utc_ms)?;
        let active = self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?;
        Ok(active.as_ref().map(|active| {
            let activity = active.state.activity_snapshot();
            CaptureLanSession {
                session_id: active.session_id.clone(),
                batch_id: active.batch_id.clone(),
                qr_svg_data_url: active.qr_svg_data_url.clone(),
                selected_address: active.selected_address.clone(),
                expires_at_utc_ms: active.state.expires_at(&activity) as f64,
                received_item_count: activity.received_item_count,
                received_bytes: activity.received_bytes as f64,
            }
        }))
    }

    pub fn stop(&self) -> Result<bool, CaptureLanError> {
        let active = self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?
            .take();
        let Some(active) = active else {
            return Ok(false);
        };
        let _ = active.shutdown.send(true);
        Ok(true)
    }

    fn stop_expired(&self, now_utc_ms: i64) -> Result<(), CaptureLanError> {
        let expired = {
            let active = self
                .active
                .lock()
                .map_err(|_| CaptureLanError::Unavailable)?;
            active
                .as_ref()
                .is_some_and(|active| active.state.is_expired(now_utc_ms))
        };
        if expired {
            let _ = self.stop()?;
        }
        Ok(())
    }
}

struct ServerState {
    session_id: String,
    context: CaptureLanContext,
    public_origin: String,
    expected_host: String,
    token_hash: [u8; 32],
    sequence_base: i64,
    started_at_utc_ms: i64,
    activity: Mutex<SessionActivity>,
    upload_slots: Arc<Semaphore>,
    shutdown: watch::Sender<bool>,
}

#[derive(Clone, Copy)]
struct SessionActivity {
    last_activity_utc_ms: i64,
    received_item_count: u32,
    received_bytes: u64,
    next_source_sequence: u32,
}

impl ServerState {
    fn activity_snapshot(&self) -> SessionActivity {
        self.activity
            .lock()
            .map(|value| *value)
            .unwrap_or(SessionActivity {
                last_activity_utc_ms: self.started_at_utc_ms,
                received_item_count: 0,
                received_bytes: 0,
                next_source_sequence: 0,
            })
    }

    fn expires_at(&self, activity: &SessionActivity) -> i64 {
        activity
            .last_activity_utc_ms
            .saturating_add(IDLE_TIMEOUT_MS)
            .min(self.started_at_utc_ms.saturating_add(ABSOLUTE_TIMEOUT_MS))
    }

    fn is_expired(&self, now_utc_ms: i64) -> bool {
        now_utc_ms >= self.expires_at(&self.activity_snapshot())
    }

    fn touch(&self, now_utc_ms: i64) {
        if let Ok(mut activity) = self.activity.lock() {
            activity.last_activity_utc_ms = now_utc_ms;
        }
    }

    fn record_upload(&self, byte_length: u64, now_utc_ms: i64) {
        if let Ok(mut activity) = self.activity.lock() {
            activity.last_activity_utc_ms = now_utc_ms;
            activity.received_item_count = activity.received_item_count.saturating_add(1);
            activity.received_bytes = activity.received_bytes.saturating_add(byte_length);
        }
    }

    fn record_source_sequence(&self, sequence: u32, now_utc_ms: i64) {
        if let Ok(mut activity) = self.activity.lock() {
            activity.last_activity_utc_ms = now_utc_ms;
            activity.next_source_sequence = activity
                .next_source_sequence
                .max(sequence.saturating_add(1));
        }
    }

    fn record_delete(&self, byte_length: u64, now_utc_ms: i64) {
        if let Ok(mut activity) = self.activity.lock() {
            activity.last_activity_utc_ms = now_utc_ms;
            activity.received_item_count = activity.received_item_count.saturating_sub(1);
            activity.received_bytes = activity.received_bytes.saturating_sub(byte_length);
        }
    }
}

async fn run_server(
    listener: TcpListener,
    state: Arc<ServerState>,
    mut shutdown_receiver: watch::Receiver<bool>,
    weak_active: Weak<Mutex<Option<ActiveSession>>>,
    session_id: String,
) {
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
        remove_active_session(&weak_active, &session_id);
        return;
    };
    let router = build_router(Arc::clone(&state));
    let watchdog_state = Arc::clone(&state);
    let watchdog = tokio::spawn(async move {
        loop {
            tokio::time::sleep(WATCHDOG_INTERVAL).await;
            if watchdog_state.is_expired(current_utc_millis()) {
                let _ = watchdog_state.shutdown.send(true);
                break;
            }
        }
    });
    let _ = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            loop {
                if *shutdown_receiver.borrow() || shutdown_receiver.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
    watchdog.abort();
    let _ = tokio::fs::remove_dir_all(session_temp_root(&state)).await;
    remove_active_session(&weak_active, &session_id);
}

fn build_router(state: Arc<ServerState>) -> Router {
    let api = Router::new()
        .route("/session", get(get_session).patch(patch_session))
        .route("/session/finish", post(finish_session))
        .route("/uploads/{client_upload_id}", put(upload_item))
        .route("/items/{item_id}/preview", get(item_preview))
        .route("/items/{item_id}", delete(delete_item))
        .layer(DefaultBodyLimit::max(
            usize::try_from(MAX_ORIGINAL_UPLOAD_BYTES).unwrap_or(usize::MAX),
        ));
    Router::new()
        .route("/mobile/", get(mobile_page))
        .route("/mobile/vendor/heic2any.js", get(heic2any_script))
        .nest("/api/v1", api)
        .with_state(state)
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
                .ok_or_else(|| ApiError::too_large())?;
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

fn collecting_batch_next_sequence(context: &CaptureLanContext) -> Result<i64, CaptureLanError> {
    let connection = context
        .connection
        .lock()
        .map_err(|_| CaptureLanError::Unavailable)?;
    let detail = get_capture_batch_detail(
        &connection,
        &context.account_id,
        &context.profile_id,
        &context.batch_id,
    )
    .map_err(|_| CaptureLanError::BatchUnavailable)?;
    if detail.batch.state != CaptureBatchState::Collecting {
        return Err(CaptureLanError::InvalidBatchState);
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(source_sequence), -1) + 1
             FROM capture_items WHERE batch_id = ?1",
            [&context.batch_id],
            |row| row.get(0),
        )
        .map_err(|_| CaptureLanError::BatchUnavailable)
}

fn private_ipv4_addresses() -> Result<Vec<CaptureLanAddress>, CaptureLanError> {
    let mut addresses = if_addrs::get_if_addrs()
        .map_err(CaptureLanError::Server)?
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) if is_private_lan(address.ip) => {
                Some(CaptureLanAddress {
                    label: interface.name,
                    address: address.ip.to_string(),
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    addresses.sort_by(|left, right| {
        left.address
            .cmp(&right.address)
            .then_with(|| left.label.cmp(&right.label))
    });
    addresses.dedup_by(|left, right| left.address == right.address);
    if addresses.is_empty() {
        return Err(CaptureLanError::NoPrivateAddress);
    }
    Ok(addresses)
}

fn select_address(
    addresses: &[CaptureLanAddress],
    selected: Option<&str>,
) -> Result<CaptureLanAddress, CaptureLanError> {
    match selected.map(str::trim).filter(|value| !value.is_empty()) {
        Some(selected) => addresses
            .iter()
            .find(|candidate| candidate.address == selected)
            .cloned()
            .ok_or(CaptureLanError::InvalidAddress),
        None if addresses.len() == 1 => Ok(addresses[0].clone()),
        None => Err(CaptureLanError::AddressRequired),
    }
}

fn is_private_lan(address: Ipv4Addr) -> bool {
    let [first, second, _, _] = address.octets();
    first == 10 || (first == 172 && (16..=31).contains(&second)) || (first == 192 && second == 168)
}

fn session_temp_root(state: &ServerState) -> PathBuf {
    state
        .context
        .blob_root
        .parent()
        .unwrap_or(&state.context.blob_root)
        .join("capture-tmp")
        .join(&state.session_id)
}

fn remove_active_session(weak_active: &Weak<Mutex<Option<ActiveSession>>>, session_id: &str) {
    if let Some(active) = weak_active.upgrade()
        && let Ok(mut active) = active.lock()
        && active
            .as_ref()
            .is_some_and(|active| active.session_id == session_id)
    {
        active.take();
    }
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
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

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        net::{SocketAddr, TcpStream},
    };

    use axum::http::{Method, Request};
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use tempfile::TempDir;
    use tower::ServiceExt as _;

    use crate::{
        infrastructure::database::{open_encrypted_database, run_migrations},
        modules::{
            capture_inbox::{
                CaptureBatchState, CreateCaptureBatch, create_capture_batch,
                get_capture_batch_detail,
            },
            profiles::{CreateProfile, create_profile},
        },
    };

    use super::*;

    const TEST_TOKEN: &str = "test-capture-token";

    struct TestServer {
        _directory: TempDir,
        state: Arc<ServerState>,
        router: Router,
    }

    impl TestServer {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("temporary directory");
            let mut connection = open_encrypted_database(
                &directory.path().join("capture-lan.db"),
                "capture-lan-test-key",
            )
            .expect("open database");
            run_migrations(&mut connection).expect("migrate database");
            let profile = create_profile(
                &mut connection,
                CreateProfile {
                    account_id: "account-1".to_owned(),
                    name: "student".to_owned(),
                    now_utc_ms: 1,
                },
            )
            .expect("create profile");
            let batch = create_capture_batch(
                &mut connection,
                CreateCaptureBatch {
                    account_id: "account-1".to_owned(),
                    profile_id: profile.id.clone(),
                    subject: "math".to_owned(),
                    state: CaptureBatchState::Collecting,
                    now_utc_ms: 2,
                },
            )
            .expect("create batch");
            let (shutdown, _) = watch::channel(false);
            let now = current_utc_millis();
            let state = Arc::new(ServerState {
                session_id: "session-1".to_owned(),
                context: CaptureLanContext {
                    connection: Arc::new(Mutex::new(connection)),
                    blob_root: directory.path().join("assets"),
                    asset_key: [91; 32],
                    account_id: "account-1".to_owned(),
                    profile_id: profile.id,
                    batch_id: batch.id,
                    notifier: Arc::new(|_| {}),
                },
                public_origin: "http://127.0.0.1:3210".to_owned(),
                expected_host: "127.0.0.1:3210".to_owned(),
                token_hash: Sha256::digest(TEST_TOKEN.as_bytes()).into(),
                sequence_base: 0,
                started_at_utc_ms: now,
                activity: Mutex::new(SessionActivity {
                    last_activity_utc_ms: now,
                    received_item_count: 0,
                    received_bytes: 0,
                    next_source_sequence: 0,
                }),
                upload_slots: Arc::new(Semaphore::new(2)),
                shutdown,
            });
            let router = build_router(Arc::clone(&state));
            Self {
                _directory: directory,
                state,
                router,
            }
        }

        fn request(&self, method: Method, uri: &str, body: Body) -> Request<Body> {
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::HOST, "127.0.0.1:3210")
                .header(header::ORIGIN, "http://127.0.0.1:3210")
                .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
                .body(body)
                .expect("request")
        }
    }

    fn png(seed: u8) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            4,
            3,
            Rgba([seed, seed.wrapping_add(1), seed.wrapping_add(2), 255]),
        ));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, ImageFormat::Png)
            .expect("encode png");
        output.into_inner()
    }

    #[test]
    fn private_network_filter_accepts_only_rfc1918_addresses() {
        assert!(is_private_lan(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(is_private_lan(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(is_private_lan(Ipv4Addr::new(172, 31, 255, 254)));
        assert!(is_private_lan(Ipv4Addr::new(192, 168, 1, 2)));
        assert!(!is_private_lan(Ipv4Addr::LOCALHOST));
        assert!(!is_private_lan(Ipv4Addr::new(169, 254, 1, 1)));
        assert!(!is_private_lan(Ipv4Addr::new(8, 8, 8, 8)));
    }

    #[test]
    fn multiple_interfaces_require_an_explicit_selection() {
        let addresses = vec![
            CaptureLanAddress {
                label: "wifi".to_owned(),
                address: "192.168.1.2".to_owned(),
            },
            CaptureLanAddress {
                label: "hotspot".to_owned(),
                address: "192.168.137.1".to_owned(),
            },
        ];
        assert!(matches!(
            select_address(&addresses, None),
            Err(CaptureLanError::AddressRequired)
        ));
        assert_eq!(
            select_address(&addresses, Some("192.168.137.1"))
                .unwrap()
                .label,
            "hotspot"
        );
        assert!(matches!(
            select_address(&addresses, Some("192.168.9.9")),
            Err(CaptureLanError::InvalidAddress)
        ));
    }

    #[test]
    fn token_comparison_checks_every_byte() {
        let left = [7_u8; 32];
        let mut different = left;
        different[31] = 8;
        assert!(constant_time_eq(&left, &left));
        assert!(!constant_time_eq(&left, &different));
    }

    #[test]
    fn api_rejects_an_invalid_token() {
        let server = TestServer::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let request = Request::builder()
            .uri("/api/v1/session")
            .header(header::HOST, "127.0.0.1:3210")
            .header(header::AUTHORIZATION, "Bearer wrong-token")
            .body(Body::empty())
            .expect("request");
        let response = runtime
            .block_on(server.router.oneshot(request))
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn api_rejects_wrong_origin_expired_sessions_and_forged_media_types() {
        let server = TestServer::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let wrong_origin = Request::builder()
            .uri("/api/v1/session")
            .header(header::HOST, "127.0.0.1:3210")
            .header(header::ORIGIN, "http://192.168.1.9:3210")
            .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
            .body(Body::empty())
            .expect("request");
        let response = runtime
            .block_on(server.router.clone().oneshot(wrong_origin))
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let mut forged = server.request(
            Method::PUT,
            &format!("/api/v1/uploads/{}", Uuid::now_v7()),
            Body::from(png(9)),
        );
        forged.headers_mut().insert(
            header::CONTENT_TYPE,
            "application/octet-stream".parse().expect("header"),
        );
        forged
            .headers_mut()
            .insert("x-source-sequence", "0".parse().expect("header"));
        let response = runtime
            .block_on(server.router.clone().oneshot(forged))
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        server
            .state
            .activity
            .lock()
            .expect("activity")
            .last_activity_utc_ms = current_utc_millis() - IDLE_TIMEOUT_MS - 1;
        let expired = server.request(Method::GET, "/api/v1/session", Body::empty());
        let response = runtime
            .block_on(server.router.clone().oneshot(expired))
            .expect("response");
        assert_eq!(response.status(), StatusCode::GONE);

        let connection = server.state.context.connection.lock().expect("connection");
        let item_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1",
                [&server.state.context.batch_id],
                |row| row.get(0),
            )
            .expect("item count");
        assert_eq!(item_count, 0);
    }

    #[test]
    fn mobile_page_hardens_headers_and_keeps_heic_decoder_lazy() {
        let server = TestServer::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let page = runtime
            .block_on(
                server.router.clone().oneshot(
                    Request::builder()
                        .uri("/mobile/")
                        .body(Body::empty())
                        .expect("page request"),
                ),
            )
            .expect("page response");
        assert_eq!(page.status(), StatusCode::OK);
        assert_eq!(
            page.headers()
                .get("x-content-type-options")
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
        assert!(page.headers().contains_key("content-security-policy"));
        assert!(!MOBILE_PAGE.contains("<script src=\"/mobile/vendor/heic2any.js\""));
        assert!(MOBILE_PAGE.contains("const createClientId="));
        assert!(MOBILE_PAGE.contains("crypto.getRandomValues"));
        assert!(MOBILE_PAGE.contains("Array.from(input.files||[])"));
        assert!(MOBILE_PAGE.contains("restoreRemoteItems"));
        assert!(MOBILE_PAGE.contains("pumpRemotePreviews"));
        assert!(MOBILE_PAGE.contains("/items/${encodeURIComponent(item.serverId)}/preview"));
        assert!(MOBILE_PAGE.contains("grid-template-columns:76px minmax(0,1fr) auto"));
        assert!(MOBILE_PAGE.contains(".item>div { min-width:0"));
        assert!(MOBILE_PAGE.contains("overflow-wrap:anywhere"));
        assert!(MOBILE_PAGE.contains("overflow-x:hidden"));
        assert!(MOBILE_PAGE.contains("已选中，正在处理图片"));

        let decoder = runtime
            .block_on(
                server.router.oneshot(
                    Request::builder()
                        .uri("/mobile/vendor/heic2any.js")
                        .body(Body::empty())
                        .expect("decoder request"),
                ),
            )
            .expect("decoder response");
        assert_eq!(decoder.status(), StatusCode::OK);
        assert_eq!(
            decoder
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/javascript; charset=utf-8")
        );
    }

    #[test]
    fn duplicate_upload_is_idempotent_and_finish_organizes_batch() {
        let server = TestServer::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let upload_id = Uuid::now_v7();
        for _ in 0..2 {
            let mut request = server.request(
                Method::PUT,
                &format!("/api/v1/uploads/{upload_id}"),
                Body::from(png(17)),
            );
            request
                .headers_mut()
                .insert("x-source-sequence", "0".parse().expect("header"));
            request
                .headers_mut()
                .insert(header::CONTENT_TYPE, "image/png".parse().expect("header"));
            request
                .headers_mut()
                .insert("x-source-name", "photo.png".parse().expect("header"));
            let response = runtime
                .block_on(server.router.clone().oneshot(request))
                .expect("upload response");
            assert_eq!(response.status(), StatusCode::OK);
        }

        let finish = server.request(Method::POST, "/api/v1/session/finish", Body::empty());
        let response = runtime
            .block_on(server.router.clone().oneshot(finish))
            .expect("finish response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let connection = server.state.context.connection.lock().expect("connection");
        let detail = get_capture_batch_detail(
            &connection,
            &server.state.context.account_id,
            &server.state.context.profile_id,
            &server.state.context.batch_id,
        )
        .expect("batch detail");
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.batch.state, CaptureBatchState::Organizing);
        assert_eq!(server.state.activity_snapshot().received_item_count, 1);
    }

    #[test]
    fn session_rehydrates_uploaded_items_and_serves_a_preview() {
        let server = TestServer::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let upload_id = Uuid::now_v7();
        let mut upload = server.request(
            Method::PUT,
            &format!("/api/v1/uploads/{upload_id}"),
            Body::from(png(23)),
        );
        upload
            .headers_mut()
            .insert("x-source-sequence", "0".parse().expect("header"));
        upload
            .headers_mut()
            .insert(header::CONTENT_TYPE, "image/png".parse().expect("header"));
        upload
            .headers_mut()
            .insert("x-source-name", "photo.png".parse().expect("header"));
        let upload_response = runtime
            .block_on(server.router.clone().oneshot(upload))
            .expect("upload response");
        assert_eq!(upload_response.status(), StatusCode::OK);
        let upload_body = runtime
            .block_on(axum::body::to_bytes(upload_response.into_body(), 1_000_000))
            .expect("upload body");
        let upload_json: serde_json::Value = serde_json::from_slice(&upload_body).expect("json");
        let item_id = upload_json["itemId"].as_str().expect("item id").to_owned();

        let session_response = runtime
            .block_on(server.router.clone().oneshot(server.request(
                Method::GET,
                "/api/v1/session",
                Body::empty(),
            )))
            .expect("session response");
        let session_body = runtime
            .block_on(axum::body::to_bytes(
                session_response.into_body(),
                1_000_000,
            ))
            .expect("session body");
        let session_json: serde_json::Value = serde_json::from_slice(&session_body).expect("json");
        assert_eq!(session_json["items"][0]["itemId"], item_id);
        assert_eq!(session_json["items"][0]["sourceName"], "photo.png");

        let preview_response = runtime
            .block_on(server.router.clone().oneshot(server.request(
                Method::GET,
                &format!("/api/v1/items/{item_id}/preview"),
                Body::empty(),
            )))
            .expect("preview response");
        assert_eq!(preview_response.status(), StatusCode::OK);
        let preview_body = runtime
            .block_on(axum::body::to_bytes(
                preview_response.into_body(),
                2_000_000,
            ))
            .expect("preview body");
        let preview_json: serde_json::Value = serde_json::from_slice(&preview_body).expect("json");
        assert!(
            preview_json["dataUrl"]
                .as_str()
                .is_some_and(|value| value.starts_with("data:image/png;base64,"))
        );
    }

    #[test]
    fn shutdown_signal_closes_the_listening_port() {
        let server = TestServer::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
        listener.set_nonblocking(true).expect("nonblocking");
        let address = listener.local_addr().expect("listener address");
        let shutdown_receiver = server.state.shutdown.subscribe();
        let weak_active = Weak::<Mutex<Option<ActiveSession>>>::new();
        let state = Arc::clone(&server.state);
        let shutdown = state.shutdown.clone();
        let thread = thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(run_server(
                    listener,
                    state,
                    shutdown_receiver,
                    weak_active,
                    "session-1".to_owned(),
                ));
        });

        wait_until_port_is_open(address);
        shutdown.send(true).expect("send shutdown");
        thread.join().expect("join server");
        assert!(TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err());
    }

    fn wait_until_port_is_open(address: SocketAddr) {
        for _ in 0..50 {
            if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("capture server did not start");
    }
}
