use serde::Deserialize;
use specta::Type;

use crate::{
    application::result::AppResult,
    infrastructure::cloud_backend::{
        self, CloudBackendError, CloudBackendKind, CloudBackendStatus,
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
pub fn sync_backend_set(request: SetBackendRequest) -> AppResult<CloudBackendStatus> {
    set_backend(request)
}

pub fn specta_commands<R: tauri::Runtime>() -> tauri_specta::Commands<R> {
    tauri_specta::collect_commands![sync_backend_status, sync_backend_set]
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
