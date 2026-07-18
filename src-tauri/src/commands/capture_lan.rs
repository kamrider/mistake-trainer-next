use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter as _, State};
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::capture_firewall::{
        CaptureFirewallError, CaptureLanPreflight,
        capture_lan_preflight as inspect_capture_lan_preflight, repair_capture_firewall,
    },
    modules::capture_lan::{
        BatchChangeNotifier, CaptureLanAddress, CaptureLanContext, CaptureLanError,
        CaptureLanManager, CaptureLanSession,
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLanStartInput {
    pub batch_id: String,
    pub selected_address: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_preflight() -> AppResult<CaptureLanPreflight> {
    firewall_result_or_error(inspect_capture_lan_preflight())
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_firewall_repair() -> AppResult<CaptureLanPreflight> {
    firewall_result_or_error(repair_capture_firewall())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureBatchChanged {
    batch_id: String,
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_addresses(
    manager: State<'_, CaptureLanManager>,
) -> AppResult<Vec<CaptureLanAddress>> {
    result_or_error(manager.addresses())
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_start(
    app: AppHandle,
    runtime: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureLanManager>,
    input: CaptureLanStartInput,
) -> AppResult<CaptureLanSession> {
    let _transition = runtime.lock_profile_transition();
    let profile = runtime.active_profile();
    let preflight = match inspect_capture_lan_preflight() {
        Ok(value) => value,
        Err(error) => return firewall_error(&error),
    };
    if preflight.needs_firewall_repair {
        return AppResult::failure(
            "capture_lan_firewall_required",
            "Windows 尚未允许手机连接。请再次点击“手机扫码”，并在系统授权窗口中选择“是”。",
            true,
            Uuid::now_v7().to_string(),
        );
    }
    let notifier_app = app.clone();
    let notifier: BatchChangeNotifier = std::sync::Arc::new(move |batch_id| {
        let _ = notifier_app.emit(
            "capture_batch_changed",
            CaptureBatchChanged {
                batch_id: batch_id.to_owned(),
            },
        );
    });
    result_or_error(manager.start(
        CaptureLanContext {
            connection: std::sync::Arc::clone(&runtime.connection),
            blob_root: runtime.blob_root.clone(),
            asset_key: runtime.asset_key,
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            batch_id: input.batch_id,
            notifier,
        },
        input.selected_address.as_deref(),
        current_utc_millis(),
    ))
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_status(
    manager: State<'_, CaptureLanManager>,
) -> AppResult<Option<CaptureLanSession>> {
    result_or_error(manager.status(current_utc_millis()))
}

#[tauri::command]
#[specta::specta]
pub fn capture_lan_stop(manager: State<'_, CaptureLanManager>) -> AppResult<bool> {
    result_or_error(manager.stop())
}

fn result_or_error<T>(result: Result<T, CaptureLanError>) -> AppResult<T> {
    match result {
        Ok(value) => AppResult::success(value),
        Err(error) => lan_error(&error),
    }
}

fn firewall_result_or_error<T>(result: Result<T, CaptureFirewallError>) -> AppResult<T> {
    match result {
        Ok(value) => AppResult::success(value),
        Err(error) => firewall_error(&error),
    }
}

fn firewall_error<T>(error: &CaptureFirewallError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        CaptureFirewallError::Cancelled => (
            "capture_lan_firewall_cancelled",
            "没有更改 Windows 权限；需要时可以再次点击“修复连接”。",
            true,
        ),
        CaptureFirewallError::Unsupported => (
            "capture_lan_firewall_unsupported",
            "当前系统不支持 Windows 局域网权限修复。",
            false,
        ),
        CaptureFirewallError::Inspection(_) => (
            "capture_lan_firewall_inspection_failed",
            "没有读取到 Windows 网络权限状态，请稍后重试。",
            true,
        ),
        CaptureFirewallError::Repair(_) => (
            "capture_lan_firewall_repair_failed",
            "Windows 没有完成连接修复，请确认管理员提示后重试。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!("capture firewall error [{diagnostic_id}] {code}: {error}");
    AppResult::failure(code, user_message, retryable, diagnostic_id)
}

fn lan_error<T>(error: &CaptureLanError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        CaptureLanError::AlreadyActive => (
            "capture_lan_already_active",
            "已经有一个手机采集会话；请先停止它再开启新的会话。",
            false,
        ),
        CaptureLanError::NoPrivateAddress => (
            "capture_lan_no_private_address",
            "没有找到可用的家庭网络或个人热点地址，请先连接 Wi‑Fi。",
            true,
        ),
        CaptureLanError::AddressRequired => (
            "capture_lan_address_required",
            "电脑连接了多个网络，请选择手机所在的那个网络。",
            false,
        ),
        CaptureLanError::InvalidAddress => (
            "capture_lan_address_invalid",
            "所选网络地址已经不可用，请刷新网络列表。",
            true,
        ),
        CaptureLanError::BatchUnavailable => (
            "capture_lan_batch_unavailable",
            "这个采集批次已经不存在，请返回采集箱刷新。",
            false,
        ),
        CaptureLanError::InvalidBatchState => (
            "capture_lan_batch_state_invalid",
            "只有仍在收图的批次可以开启手机采集。",
            false,
        ),
        CaptureLanError::Server(_) => (
            "capture_lan_server_failed",
            "手机采集服务没有启动成功，请检查网络后重试。",
            true,
        ),
        CaptureLanError::Qr => (
            "capture_lan_qr_failed",
            "二维码没有生成成功，请重新开启手机采集。",
            true,
        ),
        CaptureLanError::Unavailable => (
            "capture_lan_unavailable",
            "手机采集服务暂时不可用，请重新打开应用后重试。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!("capture LAN error [{diagnostic_id}] {code}: {error}");
    AppResult::failure(code, user_message, retryable, diagnostic_id)
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
