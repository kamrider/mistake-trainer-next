use std::fmt;

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
            Err(CloudBackendError::NotConfigured)
        }
    }
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
