use std::{
    fmt,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// The remote implementation used for optional synchronization.
///
/// The product remains usable without a remote provider. Keeping this choice in
/// Rust prevents the Vue layer from depending on a particular cloud vendor.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum CloudBackendKind {
    #[default]
    LocalOnly,
    Supabase,
    Tencent,
}

/// The configuration/status exposed to the desktop settings UI.
///
/// `configured` only means that the provider has the minimum endpoint and
/// credential environment variables. It does not imply that a provider
/// implementation is shipped in this build. `ready` is therefore false for
/// the currently reserved remote providers, which makes an accidental sync
/// attempt fail closed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackendStatus {
    pub kind: CloudBackendKind,
    pub configured: bool,
    pub ready: bool,
    pub sync_enabled: bool,
}

impl fmt::Display for CloudBackendKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LocalOnly => "local-only",
            Self::Supabase => "supabase",
            Self::Tencent => "tencent",
        })
    }
}

#[derive(Debug, Error)]
pub enum CloudBackendError {
    #[error("remote synchronization is disabled in local-only mode")]
    Disabled,
    #[error("cloud backend is not configured")]
    NotConfigured,
    #[error("cloud backend is not available in this build")]
    NotAvailable,
}

/// Provider-neutral boundary for future sync implementations.
///
/// The first implementation deliberately does not perform network I/O. This
/// makes local-first behavior explicit while leaving one seam for Supabase and
/// mainland-China providers to implement later.
pub trait CloudBackend: Send + Sync {
    fn kind(&self) -> CloudBackendKind;
    fn push_pending(&self) -> Result<u32, CloudBackendError>;
    fn pull_changes(&self) -> Result<u32, CloudBackendError>;
}

#[derive(Debug, Default)]
pub struct LocalOnlyBackend;

impl CloudBackend for LocalOnlyBackend {
    fn kind(&self) -> CloudBackendKind {
        CloudBackendKind::LocalOnly
    }

    fn push_pending(&self) -> Result<u32, CloudBackendError> {
        Err(CloudBackendError::Disabled)
    }

    fn pull_changes(&self) -> Result<u32, CloudBackendError> {
        Err(CloudBackendError::Disabled)
    }
}

pub fn backend_for(kind: CloudBackendKind) -> Result<Box<dyn CloudBackend>, CloudBackendError> {
    match kind {
        CloudBackendKind::LocalOnly => Ok(Box::new(LocalOnlyBackend)),
        CloudBackendKind::Supabase | CloudBackendKind::Tencent => {
            if !provider_is_configured(kind) {
                Err(CloudBackendError::NotConfigured)
            } else {
                // The provider ports are intentionally not implemented yet.
                // Returning a typed error here is safer than silently falling
                // back to local-only after a user selected a remote provider.
                Err(CloudBackendError::NotAvailable)
            }
        }
    }
}

fn provider_is_configured(kind: CloudBackendKind) -> bool {
    let has_non_empty = |name: &str| {
        std::env::var(name)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    };

    match kind {
        CloudBackendKind::LocalOnly => true,
        CloudBackendKind::Supabase => {
            has_non_empty("MISTAKE_TRAINER_SUPABASE_URL")
                && has_non_empty("MISTAKE_TRAINER_SUPABASE_ANON_KEY")
        }
        CloudBackendKind::Tencent => {
            has_non_empty("MISTAKE_TRAINER_TENCENT_ENDPOINT")
                && has_non_empty("MISTAKE_TRAINER_TENCENT_TOKEN")
        }
    }
}

pub fn status_for(kind: CloudBackendKind) -> CloudBackendStatus {
    match kind {
        CloudBackendKind::LocalOnly => CloudBackendStatus {
            kind,
            configured: true,
            // Local-first mode needs no network and is always usable.
            ready: true,
            sync_enabled: false,
        },
        CloudBackendKind::Supabase | CloudBackendKind::Tencent => {
            let configured = provider_is_configured(kind);
            CloudBackendStatus {
                kind,
                configured,
                // A provider adapter must be shipped before sync is enabled.
                ready: false,
                sync_enabled: false,
            }
        }
    }
}

static SELECTED_BACKEND: OnceLock<RwLock<CloudBackendKind>> = OnceLock::new();

fn selected_lock() -> &'static RwLock<CloudBackendKind> {
    SELECTED_BACKEND.get_or_init(|| RwLock::new(CloudBackendKind::LocalOnly))
}

pub fn selected_kind() -> CloudBackendKind {
    selected_lock()
        .read()
        .map(|guard| *guard)
        .unwrap_or(CloudBackendKind::LocalOnly)
}

pub fn selected_status() -> CloudBackendStatus {
    status_for(selected_kind())
}

/// Select a provider for this running app instance.
///
/// Selection is deliberately rejected when a remote provider is not fully
/// configured. This prevents a half-filled settings form from changing the
/// sync mode or causing network requests. The config is process-local until
/// the settings persistence layer is introduced; local-only remains the safe
/// default after restart.
pub fn select(kind: CloudBackendKind) -> Result<CloudBackendStatus, CloudBackendError> {
    let status = status_for(kind);
    if kind != CloudBackendKind::LocalOnly && !status.configured {
        return Err(CloudBackendError::NotConfigured);
    }

    selected_lock()
        .write()
        .map_err(|_| CloudBackendError::NotAvailable)
        .map(|mut guard| {
            *guard = kind;
            status
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_local_only() {
        assert_eq!(CloudBackendKind::default(), CloudBackendKind::LocalOnly);
        assert_eq!(CloudBackendKind::LocalOnly.to_string(), "local-only");
    }

    #[test]
    fn unconfigured_remote_backends_fail_closed() {
        assert!(matches!(
            backend_for(CloudBackendKind::Supabase),
            Err(CloudBackendError::NotConfigured)
        ));
        assert!(matches!(
            backend_for(CloudBackendKind::Tencent),
            Err(CloudBackendError::NotConfigured)
        ));
    }

    #[test]
    fn local_status_is_ready_without_network() {
        assert_eq!(
            status_for(CloudBackendKind::LocalOnly),
            CloudBackendStatus {
                kind: CloudBackendKind::LocalOnly,
                configured: true,
                ready: true,
                sync_enabled: false,
            }
        );
    }

    #[test]
    fn selecting_local_backend_is_always_safe() {
        let status = select(CloudBackendKind::LocalOnly).expect("local backend selection");
        assert_eq!(status, selected_status());
    }

    #[test]
    fn local_only_never_attempts_network_sync() {
        let backend = backend_for(CloudBackendKind::LocalOnly).expect("local backend");
        assert!(matches!(
            backend.push_pending(),
            Err(CloudBackendError::Disabled)
        ));
        assert!(matches!(
            backend.pull_changes(),
            Err(CloudBackendError::Disabled)
        ));
    }
}
