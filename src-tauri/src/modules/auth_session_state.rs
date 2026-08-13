use std::{fmt, sync::RwLock};

use crate::domain::privacy::redact_email;

use super::{AuthStatus, AuthStatusKind};

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

#[derive(Default)]
struct CloudSessionStateSnapshot {
    session: Option<ActiveCloudSession>,
    verification_email: Option<String>,
    offline: bool,
}

#[derive(Default)]
pub(super) struct CloudSessionState {
    inner: RwLock<CloudSessionStateSnapshot>,
}

impl CloudSessionState {
    pub(super) fn fmt_manager(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.inner.read().unwrap_or_else(|value| value.into_inner());
        formatter
            .debug_struct("AuthSyncManager")
            .field("session", &state.session)
            .field(
                "verification_email",
                &state.verification_email.as_deref().map(redact_email),
            )
            .finish()
    }

    pub(super) fn status(&self) -> AuthStatus {
        let state = self.inner.read().unwrap_or_else(|value| value.into_inner());
        if let Some(session) = state.session.as_ref() {
            return AuthStatus {
                kind: AuthStatusKind::Connected,
                email_hint: Some(redact_email(&session.email)),
            };
        }
        AuthStatus {
            kind: if state.offline {
                AuthStatusKind::Offline
            } else if state.verification_email.is_some() {
                AuthStatusKind::VerificationRequired
            } else {
                AuthStatusKind::SignedOut
            },
            email_hint: state.verification_email.as_deref().map(redact_email),
        }
    }

    pub(super) fn connect(
        &self,
        remote_user_id: String,
        email: String,
        access_token: String,
        expires_at_utc_ms: i64,
    ) -> AuthStatus {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|value| value.into_inner());
        state.session = Some(ActiveCloudSession {
            remote_user_id,
            email,
            access_token,
            expires_at_utc_ms,
        });
        state.verification_email = None;
        state.offline = false;
        status_from_snapshot(&state)
    }

    pub(super) fn mark_verification_required(&self, email: &str) -> AuthStatus {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|value| value.into_inner());
        state.verification_email = Some(email.to_owned());
        status_from_snapshot(&state)
    }

    pub(super) fn mark_offline(&self) -> AuthStatus {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|value| value.into_inner());
        state.offline = true;
        status_from_snapshot(&state)
    }

    pub(super) fn reject_authentication(&self) -> AuthStatus {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|value| value.into_inner());
        state.session = None;
        state.offline = false;
        status_from_snapshot(&state)
    }

    pub(super) fn access_token(&self) -> Option<String> {
        self.inner
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .session
            .as_ref()
            .map(|session| session.access_token.clone())
    }

    pub(super) fn disconnect(&self) -> AuthStatus {
        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|value| value.into_inner());
        *state = CloudSessionStateSnapshot::default();
        status_from_snapshot(&state)
    }

    pub(super) fn session_snapshot(&self) -> Option<(String, String, i64)> {
        self.inner
            .read()
            .unwrap_or_else(|value| value.into_inner())
            .session
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

fn status_from_snapshot(state: &CloudSessionStateSnapshot) -> AuthStatus {
    if let Some(session) = state.session.as_ref() {
        return AuthStatus {
            kind: AuthStatusKind::Connected,
            email_hint: Some(redact_email(&session.email)),
        };
    }
    AuthStatus {
        kind: if state.offline {
            AuthStatusKind::Offline
        } else if state.verification_email.is_some() {
            AuthStatusKind::VerificationRequired
        } else {
            AuthStatusKind::SignedOut
        },
        email_hint: state.verification_email.as_deref().map(redact_email),
    }
}
