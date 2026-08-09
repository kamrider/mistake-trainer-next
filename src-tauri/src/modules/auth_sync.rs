use std::{fmt, sync::Arc, time::Duration};

use serde::Serialize;
use specta::Type;

use crate::infrastructure::{
    cloud_backend::{self, CloudBackendKind},
    runtime::{KeyringSecretStore, SecretStore},
    supabase::{AuthReply, AuthTransport, CloudError, SupabaseClient, SupabaseConfig},
};

#[path = "auth_session_state.rs"]
mod session_state;

use session_state::CloudSessionState;

const CLOUD_REFRESH_TOKEN: &str = "cloud-refresh-token";
const CLOUD_USER_ID: &str = "cloud-user-id";
pub(crate) const CLOUD_BACKEND_KIND: &str = "cloud-backend-kind";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatusKind {
    Unconfigured,
    SignedOut,
    VerificationRequired,
    Connected,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub kind: AuthStatusKind,
    pub email_hint: Option<String>,
}

#[derive(Default)]
pub struct AuthSyncManager {
    state: CloudSessionState,
}

/// Process-scoped cloud transport state.  The publishable key is compiled into
/// the desktop build only; refresh tokens remain in the Windows credential
/// manager and never cross the Tauri/Vue boundary.
pub struct CloudAuthRuntime {
    pub client: Option<Arc<SupabaseClient>>,
    pub secrets: KeyringSecretStore,
    pub configured: bool,
}

impl CloudAuthRuntime {
    pub fn from_build_environment() -> Self {
        let (client, configured) = match SupabaseConfig::from_build_environment() {
            Ok(Some(config)) => (SupabaseClient::new(config).ok().map(Arc::new), true),
            Ok(None) | Err(_) => (None, false),
        };
        let runtime = Self {
            client,
            secrets: KeyringSecretStore::new("com.mistaketrainer.next.local-library"),
            configured,
        };
        runtime.restore_backend_selection();
        runtime
    }

    fn restore_backend_selection(&self) {
        let Ok(Some(raw_kind)) = self.secrets.get(CLOUD_BACKEND_KIND) else {
            return;
        };
        let Some(kind) = CloudBackendKind::parse(&raw_kind) else {
            return;
        };
        // A provider can be remembered before its build-time credentials are
        // present. Fail closed to local-only until the provider is configured.
        if kind == CloudBackendKind::LocalOnly || cloud_backend::status_for(kind).configured {
            let _ = cloud_backend::select(kind);
        }
    }

    pub(crate) fn persist_backend_selection(&self, kind: CloudBackendKind) -> Result<(), String> {
        self.secrets.set(CLOUD_BACKEND_KIND, &kind.to_string())
    }
}

impl fmt::Debug for AuthSyncManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.state.fmt_manager(formatter)
    }
}

impl AuthSyncManager {
    pub fn status(&self) -> AuthStatus {
        self.state.status()
    }

    pub fn accept_verified_session(
        &self,
        secrets: &dyn SecretStore,
        reply: AuthReply,
    ) -> Result<AuthStatus, CloudError> {
        let (remote_user_id, email, access_token, refresh_token, expires_at_utc_ms) =
            reply.into_session_parts()?;
        let bound_user = nonempty_secret(secrets, CLOUD_USER_ID)?;
        if bound_user
            .as_deref()
            .is_some_and(|bound| bound != remote_user_id)
        {
            return Err(CloudError::LibraryBoundToAnotherAccount);
        }
        let previous_refresh = nonempty_secret(secrets, CLOUD_REFRESH_TOKEN)?;
        secrets
            .set(CLOUD_REFRESH_TOKEN, &refresh_token)
            .map_err(|_| CloudError::SecretStore)?;
        if bound_user.is_none() && secrets.set(CLOUD_USER_ID, &remote_user_id).is_err() {
            let restored = previous_refresh.as_deref().unwrap_or("");
            let _ = secrets.set(CLOUD_REFRESH_TOKEN, restored);
            return Err(CloudError::SecretStore);
        }

        Ok(self
            .state
            .connect(remote_user_id, email, access_token, expires_at_utc_ms))
    }

    pub async fn sign_up<T: AuthTransport>(
        &self,
        transport: &T,
        secrets: &dyn SecretStore,
        email: &str,
        password: &str,
    ) -> Result<AuthStatus, CloudError> {
        let reply = transport.sign_up(email, password).await?;
        if reply.requires_email_verification() {
            return Ok(self.mark_verification_required(email));
        }
        self.accept_verified_session(secrets, reply)
    }

    pub async fn sign_in<T: AuthTransport>(
        &self,
        transport: &T,
        secrets: &dyn SecretStore,
        email: &str,
        password: &str,
    ) -> Result<AuthStatus, CloudError> {
        let reply = transport.sign_in(email, password).await?;
        self.accept_verified_session(secrets, reply)
    }

    pub async fn restore<T: AuthTransport>(
        &self,
        transport: &T,
        secrets: &dyn SecretStore,
    ) -> Result<AuthStatus, CloudError> {
        let Some(refresh_token) = nonempty_secret(secrets, CLOUD_REFRESH_TOKEN)? else {
            return Ok(self.status());
        };
        match transport.refresh(&refresh_token).await {
            Ok(reply) => self.accept_verified_session(secrets, reply),
            Err(error) if error.retryable() => Ok(self.state.mark_offline()),
            Err(CloudError::AuthenticationRejected) => {
                secrets
                    .set(CLOUD_REFRESH_TOKEN, "")
                    .map_err(|_| CloudError::SecretStore)?;
                Ok(self.state.reject_authentication())
            }
            Err(error) => Err(error),
        }
    }

    pub async fn disconnect<T: AuthTransport>(
        &self,
        transport: &T,
        secrets: &dyn SecretStore,
    ) -> Result<AuthStatus, CloudError> {
        let access_token = self.state.access_token();
        secrets
            .set(CLOUD_REFRESH_TOKEN, "")
            .map_err(|_| CloudError::SecretStore)?;
        let status = self.state.disconnect();
        if let Some(access_token) = access_token {
            // Remote revocation is bounded and best-effort. Local credentials are
            // already gone, so a slow or offline endpoint cannot block sign-out.
            let _ =
                tokio::time::timeout(Duration::from_secs(2), transport.revoke(&access_token)).await;
        }
        Ok(status)
    }

    pub fn mark_verification_required(&self, email: &str) -> AuthStatus {
        self.state.mark_verification_required(email)
    }

    pub(crate) fn session_snapshot(&self) -> Option<(String, String, i64)> {
        self.state.session_snapshot()
    }
}

fn nonempty_secret(secrets: &dyn SecretStore, name: &str) -> Result<Option<String>, CloudError> {
    secrets
        .get(name)
        .map_err(|_| CloudError::SecretStore)
        .map(|value| value.filter(|secret| !secret.is_empty()))
}
