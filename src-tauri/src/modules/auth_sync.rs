use std::{
    fmt,
    sync::{Arc, RwLock},
};

use serde::Serialize;
use specta::Type;

use crate::infrastructure::{
    runtime::{KeyringSecretStore, SecretStore},
    supabase::{
        AuthReply, AuthTransport, CloudError, SupabaseClient, SupabaseConfig, redact_email,
    },
};

const CLOUD_REFRESH_TOKEN: &str = "cloud-refresh-token";
const CLOUD_USER_ID: &str = "cloud-user-id";

struct ActiveCloudSession {
    remote_user_id: String,
    email: String,
    access_token: String,
    expires_at_utc_ms: i64,
}

impl fmt::Debug for ActiveCloudSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveCloudSession")
            .field("remote_user_id", &"<redacted>")
            .field("email", &redact_email(&self.email))
            .field("access_token", &"<redacted>")
            .field("expires_at_utc_ms", &self.expires_at_utc_ms)
            .finish()
    }
}

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
    session: RwLock<Option<ActiveCloudSession>>,
    verification_email: RwLock<Option<String>>,
    offline: RwLock<bool>,
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
        Self {
            client,
            secrets: KeyringSecretStore::new("com.mistaketrainer.next.local-library"),
            configured,
        }
    }
}

impl fmt::Debug for AuthSyncManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthSyncManager")
            .field(
                "session",
                &self
                    .session
                    .read()
                    .unwrap_or_else(|value| value.into_inner()),
            )
            .field(
                "verification_email",
                &self
                    .verification_email
                    .read()
                    .unwrap_or_else(|value| value.into_inner())
                    .as_deref()
                    .map(redact_email),
            )
            .finish()
    }
}

impl AuthSyncManager {
    pub fn status(&self) -> AuthStatus {
        if let Some(session) = self
            .session
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .as_ref()
        {
            return AuthStatus {
                kind: AuthStatusKind::Connected,
                email_hint: Some(redact_email(&session.email)),
            };
        }
        let verification = self
            .verification_email
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .clone();
        AuthStatus {
            kind: if *self
                .offline
                .read()
                .unwrap_or_else(|value| value.into_inner())
            {
                AuthStatusKind::Offline
            } else if verification.is_some() {
                AuthStatusKind::VerificationRequired
            } else {
                AuthStatusKind::SignedOut
            },
            email_hint: verification.map(|email| redact_email(&email)),
        }
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

        *self
            .session
            .write()
            .unwrap_or_else(|value| value.into_inner()) = Some(ActiveCloudSession {
            remote_user_id,
            email,
            access_token,
            expires_at_utc_ms,
        });
        *self
            .verification_email
            .write()
            .unwrap_or_else(|value| value.into_inner()) = None;
        *self
            .offline
            .write()
            .unwrap_or_else(|value| value.into_inner()) = false;
        Ok(self.status())
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
            Err(error) if error.retryable() => {
                *self
                    .offline
                    .write()
                    .unwrap_or_else(|value| value.into_inner()) = true;
                Ok(self.status())
            }
            Err(CloudError::AuthenticationRejected) => {
                secrets
                    .set(CLOUD_REFRESH_TOKEN, "")
                    .map_err(|_| CloudError::SecretStore)?;
                *self
                    .session
                    .write()
                    .unwrap_or_else(|value| value.into_inner()) = None;
                *self
                    .offline
                    .write()
                    .unwrap_or_else(|value| value.into_inner()) = false;
                Ok(self.status())
            }
            Err(error) => Err(error),
        }
    }

    pub async fn disconnect<T: AuthTransport>(
        &self,
        transport: &T,
        secrets: &dyn SecretStore,
    ) -> Result<AuthStatus, CloudError> {
        let access_token = self
            .session
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .as_ref()
            .map(|session| session.access_token.clone());
        if let Some(access_token) = access_token {
            transport.revoke(&access_token).await?;
        }
        secrets
            .set(CLOUD_REFRESH_TOKEN, "")
            .map_err(|_| CloudError::SecretStore)?;
        *self
            .session
            .write()
            .unwrap_or_else(|value| value.into_inner()) = None;
        *self
            .verification_email
            .write()
            .unwrap_or_else(|value| value.into_inner()) = None;
        *self
            .offline
            .write()
            .unwrap_or_else(|value| value.into_inner()) = false;
        Ok(self.status())
    }

    pub fn mark_verification_required(&self, email: &str) -> AuthStatus {
        *self
            .verification_email
            .write()
            .unwrap_or_else(|value| value.into_inner()) = Some(email.to_owned());
        self.status()
    }

    pub(crate) fn session_snapshot(&self) -> Option<(String, String, i64)> {
        self.session
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .as_ref()
            .map(|session| {
                (
                    session.remote_user_id.clone(),
                    session.access_token.clone(),
                    session.expires_at_utc_ms,
                )
            })
    }
}

fn nonempty_secret(secrets: &dyn SecretStore, name: &str) -> Result<Option<String>, CloudError> {
    secrets
        .get(name)
        .map_err(|_| CloudError::SecretStore)
        .map(|value| value.filter(|secret| !secret.is_empty()))
}
