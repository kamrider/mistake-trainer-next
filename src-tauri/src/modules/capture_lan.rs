use std::{
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use qrcode::{QrCode, render::svg};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use tokio::sync::{Semaphore, watch};
use uuid::Uuid;

use crate::modules::capture_inbox::{CaptureBatchState, get_capture_batch_detail};

#[path = "capture_lan_api.rs"]
mod capture_lan_api;
#[path = "capture_lan_session_registry.rs"]
mod session_registry;

use session_registry::{CaptureLanSessionRegistry, WeakCaptureLanSessionRegistry};

const IDLE_TIMEOUT_MS: i64 = 30 * 60 * 1000;
const ABSOLUTE_TIMEOUT_MS: i64 = 2 * 60 * 60 * 1000;
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);

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

#[derive(Clone, Default)]
pub struct CaptureLanManager {
    sessions: CaptureLanSessionRegistry,
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
        self.sessions.shutdown_if_last_owner();
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
        self.sessions.ensure_startable(now_utc_ms)?;

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

        self.sessions.install(
            session_id.clone(),
            state.context.batch_id.clone(),
            selected.address,
            qr_svg_data_url,
            Arc::clone(&state),
            shutdown,
        )?;

        let weak_sessions = self.sessions.downgrade();
        let thread_session_id = session_id.clone();
        let server_state = Arc::clone(&state);
        let handle = thread::Builder::new()
            .name("capture-lan-server".to_owned())
            .spawn(move || {
                runtime.block_on(run_server(
                    listener,
                    server_state,
                    shutdown_receiver,
                    weak_sessions,
                    thread_session_id,
                ));
            });
        match handle {
            Ok(handle) => drop(handle),
            Err(error) => {
                let _ = self.sessions.remove_if_session(&session_id);
                return Err(CaptureLanError::Server(error));
            }
        }

        self.status(now_utc_ms)?.ok_or(CaptureLanError::Unavailable)
    }

    pub fn status(&self, now_utc_ms: i64) -> Result<Option<CaptureLanSession>, CaptureLanError> {
        self.sessions.status(now_utc_ms)
    }

    pub fn stop(&self) -> Result<bool, CaptureLanError> {
        self.sessions.stop()
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
    weak_sessions: WeakCaptureLanSessionRegistry,
    session_id: String,
) {
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
        weak_sessions.remove_if_session(&session_id);
        return;
    };
    let router = capture_lan_api::build_router(Arc::clone(&state));
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
    weak_sessions.remove_if_session(&session_id);
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

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
