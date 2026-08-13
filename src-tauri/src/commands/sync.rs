use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::cloud_backend::{
        self, CloudBackendError, CloudBackendKind, CloudBackendStatus,
    },
    modules::auth_sync::{AuthStatus, AuthStatusKind, AuthSyncManager, CloudAuthRuntime},
    modules::{
        capture_lan::CaptureLanManager,
        profiles::list_profiles,
        sync_conflicts::{
            ResolveSyncConflictEntityInput, ResolveSyncConflictFieldInput, SyncConflictError,
            SyncConflictSummary, list_sync_conflicts, resolve_sync_conflict_entity,
            resolve_sync_conflict_field,
        },
        sync_coordinator::SyncCoordinator,
        sync_pull::pull_until_current,
        sync_push::push_once,
    },
};

/// DTO used by the generated command client when changing the sync provider.
/// Keeping this as a named request leaves room for non-secret provider options
/// (for example a region) without exposing credentials to the Vue layer.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetBackendRequest {
    pub kind: CloudBackendKind,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthCredentials {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CloudAuthState {
    pub configured: bool,
    pub status: AuthStatus,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncNowReport {
    pub pushed_operation_count: u32,
    pub uploaded_asset_count: u32,
    pub pulled_change_count: u32,
    pub downloaded_asset_count: u32,
    pub final_cursor: f64,
}

pub fn backend_status() -> AppResult<CloudBackendStatus> {
    AppResult::success(cloud_backend::selected_status())
}

pub fn set_backend(request: SetBackendRequest) -> AppResult<CloudBackendStatus> {
    match cloud_backend::select(request.kind) {
        Ok(status) => AppResult::success(status),
        Err(error) => AppResult::failure(
            error_code(&error),
            user_message(&error),
            false,
            "sync-backend-selection",
        ),
    }
}

pub fn set_backend_persisted(
    runtime: &CloudAuthRuntime,
    request: SetBackendRequest,
) -> AppResult<CloudBackendStatus> {
    let previous = cloud_backend::selected_kind();
    let result = set_backend(request.clone());
    if matches!(result, AppResult::Failure { .. }) {
        return result;
    }
    if runtime.persist_backend_selection(request.kind).is_err() {
        let _ = cloud_backend::select(previous);
        return AppResult::failure(
            "SYNC_BACKEND_PERSIST_FAILED",
            "同步设置没有保存成功，已恢复到之前的模式；本地数据不受影响",
            true,
            "sync-backend-persist",
        );
    }
    result
}

pub fn auth_status(
    manager: &AuthSyncManager,
    runtime: &CloudAuthRuntime,
) -> AppResult<CloudAuthState> {
    let status = if runtime.configured {
        manager.status()
    } else {
        AuthStatus {
            kind: AuthStatusKind::Unconfigured,
            email_hint: None,
        }
    };
    AppResult::success(CloudAuthState {
        configured: runtime.configured,
        status,
    })
}

fn auth_unconfigured() -> AppResult<CloudAuthState> {
    AppResult::failure(
        "AUTH_UNCONFIGURED",
        "此版本未配置云端地址，应用仍可完全离线使用",
        false,
        "auth-unconfigured",
    )
}

fn auth_failure(error: crate::infrastructure::supabase::CloudError) -> AppResult<CloudAuthState> {
    let (code, message) = match error {
        crate::infrastructure::supabase::CloudError::InvalidCredentialsInput => {
            ("AUTH_INVALID_INPUT", "请输入有效邮箱和至少 8 位密码")
        }
        crate::infrastructure::supabase::CloudError::AuthenticationRejected => {
            ("AUTH_REJECTED", "邮箱或密码不正确，或账户尚未允许登录")
        }
        crate::infrastructure::supabase::CloudError::EmailVerificationRequired => {
            ("AUTH_VERIFICATION_REQUIRED", "请先完成邮箱验证，再登录")
        }
        crate::infrastructure::supabase::CloudError::LibraryBoundToAnotherAccount => {
            ("AUTH_ACCOUNT_BOUND", "此本地库已经绑定另一个云端账户")
        }
        crate::infrastructure::supabase::CloudError::SecretStore => (
            "AUTH_SECRET_STORE",
            "Windows 凭据保存失败，请检查系统凭据服务",
        ),
        crate::infrastructure::supabase::CloudError::Timeout => {
            ("AUTH_TIMEOUT", "云端请求超时，请稍后重试")
        }
        crate::infrastructure::supabase::CloudError::Network => {
            ("AUTH_NETWORK", "无法连接云端，已保留本地离线模式")
        }
        _ => ("AUTH_REQUEST_FAILED", "云端登录暂时失败，请稍后重试"),
    };
    AppResult::failure(code, message, error.retryable(), "auth-request")
}

fn error_code(error: &CloudBackendError) -> &'static str {
    match error {
        CloudBackendError::Disabled => "SYNC_DISABLED",
        CloudBackendError::NotConfigured => "SYNC_BACKEND_NOT_CONFIGURED",
        CloudBackendError::NotAvailable => "SYNC_BACKEND_NOT_AVAILABLE",
    }
}

fn user_message(error: &CloudBackendError) -> &'static str {
    match error {
        CloudBackendError::Disabled => "当前为仅本地模式，未启用云端同步",
        CloudBackendError::NotConfigured => "该同步服务尚未配置，已保持本地模式",
        CloudBackendError::NotAvailable => "该同步服务暂未在此版本启用，已保持本地模式",
    }
}

#[tauri::command]
#[specta::specta]
pub fn sync_backend_status() -> AppResult<CloudBackendStatus> {
    backend_status()
}

#[tauri::command]
#[specta::specta]
pub fn sync_backend_set(
    runtime: State<'_, CloudAuthRuntime>,
    request: SetBackendRequest,
) -> AppResult<CloudBackendStatus> {
    set_backend_persisted(&runtime, request)
}

#[tauri::command]
#[specta::specta]
pub fn auth_status_command(
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
) -> AppResult<CloudAuthState> {
    auth_status(&manager, &runtime)
}

#[tauri::command]
#[specta::specta]
pub async fn auth_sign_up(
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
    request: AuthCredentials,
) -> Result<AppResult<CloudAuthState>, ()> {
    let Some(client) = runtime.client.as_ref() else {
        return Ok(auth_unconfigured());
    };
    match manager
        .sign_up(
            client.as_ref(),
            &runtime.secrets,
            &request.email,
            &request.password,
        )
        .await
    {
        Ok(status) => Ok(AppResult::success(CloudAuthState {
            configured: true,
            status,
        })),
        Err(error) => Ok(auth_failure(error)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn auth_sign_in(
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
    request: AuthCredentials,
) -> Result<AppResult<CloudAuthState>, ()> {
    let Some(client) = runtime.client.as_ref() else {
        return Ok(auth_unconfigured());
    };
    match manager
        .sign_in(
            client.as_ref(),
            &runtime.secrets,
            &request.email,
            &request.password,
        )
        .await
    {
        Ok(status) => Ok(AppResult::success(CloudAuthState {
            configured: true,
            status,
        })),
        Err(error) => Ok(auth_failure(error)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn auth_restore(
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
) -> Result<AppResult<CloudAuthState>, ()> {
    let Some(client) = runtime.client.as_ref() else {
        return Ok(AppResult::success(CloudAuthState {
            configured: false,
            status: AuthStatus {
                kind: AuthStatusKind::Unconfigured,
                email_hint: None,
            },
        }));
    };
    match manager.restore(client.as_ref(), &runtime.secrets).await {
        Ok(status) => Ok(AppResult::success(CloudAuthState {
            configured: true,
            status,
        })),
        Err(error) => Ok(auth_failure(error)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn auth_disconnect(
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
) -> Result<AppResult<CloudAuthState>, ()> {
    let Some(client) = runtime.client.as_ref() else {
        return Ok(auth_unconfigured());
    };
    match manager.disconnect(client.as_ref(), &runtime.secrets).await {
        Ok(status) => Ok(AppResult::success(CloudAuthState {
            configured: true,
            status,
        })),
        Err(error) => Ok(auth_failure(error)),
    }
}

fn sync_is_retryable(code: &str) -> bool {
    matches!(
        code,
        "cloud_timeout" | "cloud_network" | "cloud_rate_limited" | "cloud_unavailable"
    )
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncAdmissionError {
    AlreadyRunning,
    CaptureActive,
}

impl SyncAdmissionError {
    const fn stable_code(self) -> &'static str {
        match self {
            Self::AlreadyRunning => "sync_already_running",
            Self::CaptureActive => "sync_capture_active",
        }
    }
}

fn sync_admission(permit_acquired: bool, capture_active: bool) -> Result<(), SyncAdmissionError> {
    if !permit_acquired {
        return Err(SyncAdmissionError::AlreadyRunning);
    }
    if capture_active {
        return Err(SyncAdmissionError::CaptureActive);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn sync_now(
    state: State<'_, crate::infrastructure::runtime::LibraryRuntime>,
    manager: State<'_, AuthSyncManager>,
    runtime: State<'_, CloudAuthRuntime>,
    capture_lan: State<'_, CaptureLanManager>,
    coordinator: State<'_, SyncCoordinator>,
) -> Result<AppResult<SyncNowReport>, ()> {
    let Some(client) = runtime.client.as_ref().map(Arc::clone) else {
        return Ok(AppResult::failure(
            "SYNC_UNCONFIGURED",
            "此版本未配置云端地址，当前仍使用本地离线模式",
            false,
            "sync-unconfigured",
        ));
    };
    let Some((remote_user_id, access_token, expires_at_utc_ms)) = manager.session_snapshot() else {
        return Ok(AppResult::failure(
            "SYNC_SIGNED_OUT",
            "请先登录云端账户，再开始同步",
            false,
            "sync-signed-out",
        ));
    };
    if expires_at_utc_ms <= current_utc_millis() {
        return Ok(AppResult::failure(
            "SYNC_SESSION_EXPIRED",
            "云端登录已过期，请重新登录",
            true,
            "sync-session-expired",
        ));
    }

    let permit = coordinator.try_begin();
    if sync_admission(permit.is_some(), false).is_err() {
        return Ok(AppResult::failure(
            "SYNC_ALREADY_RUNNING",
            "同步已经在进行，请稍候。",
            true,
            "sync-already-running",
        ));
    }
    let _permit = permit.expect("sync admission accepted an available permit");

    let connection = Arc::clone(&state.connection);
    let profile_transition = state.profile_transition_lock();
    let capture_lan = capture_lan.inner().clone();
    let blob_root = state.blob_root.clone();
    let asset_key = state.asset_key;
    let account_id = state.account_id().to_owned();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let _profile_guard = profile_transition
            .lock()
            .map_err(|_| "sync_profile_transition_locked")?;
        let capture_active = capture_lan
            .status(current_utc_millis())
            .map_err(|_| "sync_capture_status_failed")?
            .is_some();
        sync_admission(true, capture_active).map_err(SyncAdmissionError::stable_code)?;
        let mut connection = connection.lock().map_err(|_| "sync_database_locked")?;
        let runtime = tokio::runtime::Runtime::new().map_err(|_| "sync_runtime_failed")?;

        runtime.block_on(async {
            let asset_decryptor =
                crate::infrastructure::assets::KeyedAssetDecryptor::new(&asset_key);
            let asset_encryptor =
                crate::infrastructure::assets::KeyedAssetEncryptor::new(&asset_key);
            let asset_blob_remover = crate::infrastructure::assets::FilesystemAssetBlobRemover;
            let pushed = push_once(
                &mut connection,
                client.as_ref(),
                &account_id,
                &remote_user_id,
                &access_token,
                &blob_root,
                &asset_decryptor,
                current_utc_millis(),
            )
            .await
            .map_err(|error| error.stable_code())?;
            let pulled = pull_until_current(
                &mut connection,
                client.as_ref(),
                &account_id,
                &remote_user_id,
                &access_token,
                &blob_root,
                &asset_encryptor,
                &asset_blob_remover,
                current_utc_millis(),
            )
            .await
            .map_err(|error| error.stable_code())?;
            Ok::<_, &'static str>(SyncNowReport {
                pushed_operation_count: u32::try_from(pushed.acknowledged_operation_ids.len())
                    .unwrap_or(u32::MAX),
                uploaded_asset_count: u32::try_from(pushed.uploaded_asset_ids.len())
                    .unwrap_or(u32::MAX),
                pulled_change_count: pulled.applied_count,
                downloaded_asset_count: pulled.downloaded_asset_count,
                final_cursor: pulled.final_cursor as f64,
            })
        })
    });
    match worker.await {
        Ok(Ok(report)) => {
            let current = state.active_profile();
            if let Ok(connection) = state.connection.lock()
                && let Ok(profiles) = list_profiles(&connection, state.account_id())
                && let Some(active) = profiles
                    .iter()
                    .find(|profile| profile.id == current.id)
                    .or_else(|| profiles.first())
            {
                state.replace_active_profile(active);
            }
            Ok(AppResult::success(report))
        }
        Ok(Err("sync_capture_active")) => Ok(AppResult::failure(
            "SYNC_CAPTURE_ACTIVE",
            "手机采集正在进行；结束拍摄后，应用回到前台或网络恢复时会继续同步，当前上传不会被打断。",
            true,
            "sync-capture-active",
        )),
        Ok(Err(code)) => Ok(AppResult::failure(
            code,
            "同步未完成，本地数据保持不变；请稍后重试或查看诊断信息",
            sync_is_retryable(code),
            "sync-now",
        )),
        Err(_) => Ok(AppResult::failure(
            "SYNC_WORKER_FAILED",
            "同步任务意外中断，本地数据保持不变",
            true,
            "sync-worker",
        )),
    }
}

#[tauri::command]
#[specta::specta]
pub fn sync_conflict_list(
    state: State<'_, crate::infrastructure::runtime::LibraryRuntime>,
) -> AppResult<Vec<SyncConflictSummary>> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return sync_conflict_failure(SyncConflictError::NotFound),
    };
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => return sync_conflict_failure(SyncConflictError::Database(error)),
    };
    match list_sync_conflicts(&transaction, state.account_id(), &profile.id) {
        Ok(conflicts) => AppResult::success(conflicts),
        Err(error) => sync_conflict_failure(error),
    }
}

#[tauri::command]
#[specta::specta]
pub fn sync_conflict_resolve(
    state: State<'_, crate::infrastructure::runtime::LibraryRuntime>,
    input: ResolveSyncConflictFieldInput,
) -> AppResult<Vec<SyncConflictSummary>> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return sync_conflict_failure(SyncConflictError::NotFound),
    };
    let result = resolve_sync_conflict_field(
        &mut connection,
        state.account_id(),
        &profile.id,
        input,
        current_utc_millis(),
    );
    drop(connection);
    if result.is_ok() {
        refresh_active_profile(&state);
    }
    match result {
        Ok(conflicts) => AppResult::success(conflicts),
        Err(error) => sync_conflict_failure(error),
    }
}

#[tauri::command]
#[specta::specta]
pub fn sync_conflict_resolve_entity(
    state: State<'_, crate::infrastructure::runtime::LibraryRuntime>,
    input: ResolveSyncConflictEntityInput,
) -> AppResult<Vec<SyncConflictSummary>> {
    let profile = state.active_profile();
    let mut connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return sync_conflict_failure(SyncConflictError::NotFound),
    };
    let result = resolve_sync_conflict_entity(
        &mut connection,
        state.account_id(),
        &profile.id,
        input,
        current_utc_millis(),
    );
    drop(connection);
    if result.is_ok() {
        refresh_active_profile(&state);
    }
    match result {
        Ok(conflicts) => AppResult::success(conflicts),
        Err(error) => sync_conflict_failure(error),
    }
}

fn refresh_active_profile(state: &State<'_, crate::infrastructure::runtime::LibraryRuntime>) {
    let Ok(connection) = state.connection.lock() else {
        return;
    };
    let Ok(profiles) = list_profiles(&connection, state.account_id()) else {
        return;
    };
    let current_id = state.active_profile().id;
    if let Some(active) = profiles
        .iter()
        .find(|candidate| candidate.id == current_id)
        .or_else(|| profiles.first())
    {
        state.replace_active_profile(active);
    }
}

fn sync_conflict_failure<T>(error: SyncConflictError) -> AppResult<T> {
    let (code, message) = match error {
        SyncConflictError::NotFound => (
            "SYNC_CONFLICT_NOT_FOUND",
            "这条冲突已处理或不属于当前学习档案，请刷新后再试。",
        ),
        SyncConflictError::InvalidValue => (
            "SYNC_CONFLICT_VALUE_INVALID",
            "所选版本的数据格式无效，未修改本地内容。",
        ),
        SyncConflictError::LastProfile => (
            "SYNC_CONFLICT_LAST_PROFILE",
            "不能采用删除版本：账户至少需要保留一个学习档案。",
        ),
        _ => (
            "SYNC_CONFLICT_OPERATION_FAILED",
            "冲突暂时无法处理，本地内容保持不变，请稍后重试。",
        ),
    };
    AppResult::failure(code, message, false, Uuid::now_v7().to_string())
}

pub fn specta_commands<R: tauri::Runtime>() -> tauri_specta::Commands<R> {
    tauri_specta::collect_commands![
        sync_backend_status,
        sync_backend_set,
        auth_status_command,
        auth_sign_up,
        auth_sign_in,
        auth_restore,
        auth_disconnect,
        sync_now,
        sync_conflict_list,
        sync_conflict_resolve,
        sync_conflict_resolve_entity
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_capture_defers_sync_without_stopping_it() {
        assert_eq!(
            sync_admission(true, true),
            Err(SyncAdmissionError::CaptureActive)
        );
    }

    #[test]
    fn a_running_sync_rejects_a_duplicate() {
        assert_eq!(
            sync_admission(false, false),
            Err(SyncAdmissionError::AlreadyRunning)
        );
    }

    #[test]
    fn default_status_is_local_only() {
        let result = backend_status();
        match result {
            AppResult::Success { data, .. } => {
                assert_eq!(data.kind, CloudBackendKind::LocalOnly);
                assert!(data.configured);
                assert!(data.ready);
                assert!(!data.sync_enabled);
            }
            AppResult::Failure { .. } => panic!("status should always be available"),
        }
    }

    #[test]
    fn remote_selection_without_credentials_is_safe() {
        let result = set_backend(SetBackendRequest {
            kind: CloudBackendKind::Supabase,
        });
        match result {
            AppResult::Failure { error, .. } => {
                assert_eq!(error.code, "SYNC_BACKEND_NOT_CONFIGURED");
                assert!(!error.retryable);
            }
            AppResult::Success { .. } => panic!("unconfigured provider must fail closed"),
        }
    }

    #[test]
    fn auth_status_fails_closed_when_supabase_is_not_configured() {
        let manager = AuthSyncManager::default();
        let runtime = CloudAuthRuntime {
            client: None,
            secrets: crate::infrastructure::runtime::KeyringSecretStore::new(
                "com.mistaketrainer.next.test",
            ),
            configured: false,
        };
        let AppResult::Success { data, .. } = auth_status(&manager, &runtime) else {
            panic!("status should always be available")
        };
        assert!(!data.configured);
        assert_eq!(data.status.kind, AuthStatusKind::Unconfigured);
    }
}
