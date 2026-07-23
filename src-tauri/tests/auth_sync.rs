use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::Mutex,
};
use tokio::sync::Notify;

use mistake_trainer_next_lib::{
    infrastructure::{
        runtime::SecretStore,
        supabase::{AuthReply, AuthTransport, CloudError},
    },
    modules::auth_sync::{AuthStatusKind, AuthSyncManager},
};

#[derive(Default)]
struct MemorySecrets {
    values: Mutex<HashMap<String, String>>,
    fail_on: Mutex<Option<String>>,
}

#[derive(Default)]
struct QueueTransport {
    replies: Mutex<VecDeque<Result<AuthReply, CloudError>>>,
    revokes: Mutex<VecDeque<Result<(), CloudError>>>,
}

impl QueueTransport {
    fn with_reply(reply: Result<AuthReply, CloudError>) -> Self {
        Self {
            replies: Mutex::new(VecDeque::from([reply])),
            revokes: Mutex::new(VecDeque::new()),
        }
    }

    fn next_reply(&self) -> Result<AuthReply, CloudError> {
        self.replies.lock().unwrap().pop_front().unwrap()
    }
}

impl AuthTransport for QueueTransport {
    fn sign_up<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        let result = self.next_reply();
        async move { result }
    }

    fn sign_in<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        let result = self.next_reply();
        async move { result }
    }

    fn refresh<'a>(
        &'a self,
        _refresh_token: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        let result = self.next_reply();
        async move { result }
    }

    fn revoke<'a>(
        &'a self,
        _access_token: &'a str,
    ) -> impl Future<Output = Result<(), CloudError>> + Send + 'a {
        let result = self.revokes.lock().unwrap().pop_front().unwrap_or(Ok(()));
        async move { result }
    }
}

#[derive(Default)]
struct BlockingRevokeTransport {
    entered: Notify,
    release: Notify,
}

impl AuthTransport for BlockingRevokeTransport {
    fn sign_up<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async { panic!("sign_up is not used by this test") }
    }

    fn sign_in<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async { panic!("sign_in is not used by this test") }
    }

    fn refresh<'a>(
        &'a self,
        _refresh_token: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async { panic!("refresh is not used by this test") }
    }

    fn revoke<'a>(
        &'a self,
        _access_token: &'a str,
    ) -> impl Future<Output = Result<(), CloudError>> + Send + 'a {
        async move {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(())
        }
    }
}

impl MemorySecrets {
    fn value(&self, key: &str) -> Option<String> {
        self.values.lock().unwrap().get(key).cloned()
    }

    fn fail_next_set(&self, key: &str) {
        *self.fail_on.lock().unwrap() = Some(key.to_owned());
    }
}

impl SecretStore for MemorySecrets {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.value(name))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        {
            let mut fail_on = self.fail_on.lock().unwrap();
            if fail_on.as_deref() == Some(name) {
                *fail_on = None;
                return Err("injected credential failure".to_owned());
            }
        }
        self.values
            .lock()
            .unwrap()
            .insert(name.to_owned(), value.to_owned());
        Ok(())
    }
}

#[test]
fn first_verified_session_binds_the_library_and_rotates_only_secrets() {
    let manager = AuthSyncManager::default();
    let secrets = MemorySecrets::default();
    let reply = AuthReply::verified_session(
        "33333333-3333-4333-8333-333333333333",
        "student@example.test",
        "access-secret",
        "refresh-secret",
        1_800_000_000_000,
    );

    let status = manager.accept_verified_session(&secrets, reply).unwrap();

    assert_eq!(status.kind, AuthStatusKind::Connected);
    assert_eq!(status.email_hint.as_deref(), Some("s***t@example.test"));
    assert_eq!(
        secrets.value("cloud-user-id").as_deref(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(
        secrets.value("cloud-refresh-token").as_deref(),
        Some("refresh-secret")
    );
    let debug = format!("{manager:?}");
    assert!(!debug.contains("access-secret"));
    assert!(!debug.contains("refresh-secret"));
}

#[test]
fn another_remote_user_cannot_rebind_an_existing_library() {
    let manager = AuthSyncManager::default();
    let secrets = MemorySecrets::default();
    manager
        .accept_verified_session(
            &secrets,
            AuthReply::verified_session(
                "33333333-3333-4333-8333-333333333333",
                "first@example.test",
                "first-access",
                "first-refresh",
                1_800_000_000_000,
            ),
        )
        .unwrap();

    let result = manager.accept_verified_session(
        &secrets,
        AuthReply::verified_session(
            "44444444-4444-4444-8444-444444444444",
            "second@example.test",
            "second-access",
            "second-refresh",
            1_800_000_100_000,
        ),
    );

    assert!(matches!(
        result,
        Err(CloudError::LibraryBoundToAnotherAccount)
    ));
    assert_eq!(
        secrets.value("cloud-refresh-token").as_deref(),
        Some("first-refresh")
    );
    assert_eq!(
        manager.status().email_hint.as_deref(),
        Some("f***t@example.test")
    );
}

#[test]
fn binding_failure_restores_the_previous_refresh_token_and_memory_session() {
    let manager = AuthSyncManager::default();
    let secrets = MemorySecrets::default();
    secrets
        .set("cloud-refresh-token", "previous-refresh")
        .unwrap();
    secrets.fail_next_set("cloud-user-id");

    let result = manager.accept_verified_session(
        &secrets,
        AuthReply::verified_session(
            "33333333-3333-4333-8333-333333333333",
            "student@example.test",
            "new-access",
            "new-refresh",
            1_800_000_000_000,
        ),
    );

    assert!(matches!(result, Err(CloudError::SecretStore)));
    assert_eq!(
        secrets.value("cloud-refresh-token").as_deref(),
        Some("previous-refresh")
    );
    assert_eq!(manager.status().kind, AuthStatusKind::SignedOut);
}

#[test]
fn malformed_remote_user_id_is_rejected_before_writing_secrets() {
    let manager = AuthSyncManager::default();
    let secrets = MemorySecrets::default();

    let result = manager.accept_verified_session(
        &secrets,
        AuthReply::verified_session(
            "not-a-uuid",
            "student@example.test",
            "access-secret",
            "refresh-secret",
            1_800_000_000_000,
        ),
    );

    assert!(matches!(result, Err(CloudError::InvalidResponse)));
    assert_eq!(secrets.value("cloud-user-id"), None);
    assert_eq!(secrets.value("cloud-refresh-token"), None);
}

#[test]
fn signup_without_a_session_waits_for_verification_without_binding_the_library() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        let transport = QueueTransport::with_reply(Ok(AuthReply::verification_required(
            "33333333-3333-4333-8333-333333333333",
            "student@example.test",
        )));

        let status = manager
            .sign_up(&transport, &secrets, "student@example.test", "password-123")
            .await
            .unwrap();

        assert_eq!(status.kind, AuthStatusKind::VerificationRequired);
        assert_eq!(status.email_hint.as_deref(), Some("s***t@example.test"));
        assert_eq!(secrets.value("cloud-user-id"), None);
        assert_eq!(secrets.value("cloud-refresh-token"), None);
    });
}

#[test]
fn retryable_startup_refresh_failure_preserves_the_token_and_enters_offline_state() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        secrets
            .set("cloud-refresh-token", "still-valid-refresh")
            .unwrap();
        secrets
            .set("cloud-user-id", "33333333-3333-4333-8333-333333333333")
            .unwrap();
        let transport = QueueTransport::with_reply(Err(CloudError::Network));

        let status = manager.restore(&transport, &secrets).await.unwrap();

        assert_eq!(status.kind, AuthStatusKind::Offline);
        assert_eq!(
            secrets.value("cloud-refresh-token").as_deref(),
            Some("still-valid-refresh")
        );
    });
}

#[test]
fn rejected_refresh_clears_offline_state_and_stale_refresh_token() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        secrets
            .set("cloud-refresh-token", "expired-refresh")
            .unwrap();
        let offline = QueueTransport::with_reply(Err(CloudError::Network));
        assert_eq!(
            manager.restore(&offline, &secrets).await.unwrap().kind,
            AuthStatusKind::Offline
        );

        let rejected = QueueTransport::with_reply(Err(CloudError::AuthenticationRejected));
        let status = manager.restore(&rejected, &secrets).await.unwrap();

        assert_eq!(status.kind, AuthStatusKind::SignedOut);
        assert_eq!(secrets.value("cloud-refresh-token").as_deref(), Some(""));
    });
}

#[test]
fn disconnect_revokes_the_session_and_clears_only_the_refresh_token() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        manager
            .accept_verified_session(
                &secrets,
                AuthReply::verified_session(
                    "33333333-3333-4333-8333-333333333333",
                    "student@example.test",
                    "access-secret",
                    "refresh-secret",
                    1_800_000_000_000,
                ),
            )
            .unwrap();
        let transport = QueueTransport::default();

        let status = manager.disconnect(&transport, &secrets).await.unwrap();

        assert_eq!(status.kind, AuthStatusKind::SignedOut);
        assert_eq!(secrets.value("cloud-refresh-token").as_deref(), Some(""));
        assert_eq!(
            secrets.value("cloud-user-id").as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
    });
}

#[test]
fn disconnect_clears_the_local_session_when_remote_revocation_is_offline() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        manager
            .accept_verified_session(
                &secrets,
                AuthReply::verified_session(
                    "33333333-3333-4333-8333-333333333333",
                    "student@example.test",
                    "access-secret",
                    "refresh-secret",
                    1_800_000_000_000,
                ),
            )
            .unwrap();
        let transport = QueueTransport {
            replies: Mutex::new(VecDeque::new()),
            revokes: Mutex::new(VecDeque::from([Err(CloudError::Network)])),
        };

        let status = manager.disconnect(&transport, &secrets).await.unwrap();

        assert_eq!(status.kind, AuthStatusKind::SignedOut);
        assert_eq!(secrets.value("cloud-refresh-token").as_deref(), Some(""));
        assert_eq!(
            secrets.value("cloud-user-id").as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
    });
}

#[test]
fn disconnect_clears_local_credentials_before_remote_revocation_finishes() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let manager = AuthSyncManager::default();
        let secrets = MemorySecrets::default();
        manager
            .accept_verified_session(
                &secrets,
                AuthReply::verified_session(
                    "33333333-3333-4333-8333-333333333333",
                    "student@example.test",
                    "access-secret",
                    "refresh-secret",
                    1_800_000_000_000,
                ),
            )
            .unwrap();
        let transport = BlockingRevokeTransport::default();
        let disconnect = manager.disconnect(&transport, &secrets);
        tokio::pin!(disconnect);

        tokio::select! {
            _ = transport.entered.notified() => {}
            result = &mut disconnect => panic!("revoke unexpectedly completed: {result:?}"),
        }

        assert_eq!(manager.status().kind, AuthStatusKind::SignedOut);
        assert_eq!(secrets.value("cloud-refresh-token").as_deref(), Some(""));

        transport.release.notify_one();
        assert_eq!(disconnect.await.unwrap().kind, AuthStatusKind::SignedOut);
    });
}
