use std::sync::{Arc, Mutex, Weak};

use tokio::sync::watch;

use super::{CaptureLanError, CaptureLanSession, ServerState};

struct ActiveSession {
    session_id: String,
    batch_id: String,
    selected_address: String,
    qr_svg_data_url: String,
    state: Arc<ServerState>,
    shutdown: watch::Sender<bool>,
}

#[derive(Clone, Default)]
pub(super) struct CaptureLanSessionRegistry {
    active: Arc<Mutex<Option<ActiveSession>>>,
}

#[derive(Clone, Default)]
pub(super) struct WeakCaptureLanSessionRegistry {
    active: Weak<Mutex<Option<ActiveSession>>>,
}

impl CaptureLanSessionRegistry {
    pub(super) fn ensure_startable(&self, now_utc_ms: i64) -> Result<(), CaptureLanError> {
        let expired = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| CaptureLanError::Unavailable)?;
            let expired = active
                .as_ref()
                .is_some_and(|active| active.state.is_expired(now_utc_ms));
            if active.is_some() && !expired {
                return Err(CaptureLanError::AlreadyActive);
            }
            if expired { active.take() } else { None }
        };
        signal_shutdown(expired);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn install(
        &self,
        session_id: String,
        batch_id: String,
        selected_address: String,
        qr_svg_data_url: String,
        state: Arc<ServerState>,
        shutdown: watch::Sender<bool>,
    ) -> Result<(), CaptureLanError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?;
        if active.is_some() {
            return Err(CaptureLanError::AlreadyActive);
        }
        *active = Some(ActiveSession {
            session_id,
            batch_id,
            selected_address,
            qr_svg_data_url,
            state,
            shutdown,
        });
        Ok(())
    }

    pub(super) fn status(
        &self,
        now_utc_ms: i64,
    ) -> Result<Option<CaptureLanSession>, CaptureLanError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?;
        if active
            .as_ref()
            .is_some_and(|active| active.state.is_expired(now_utc_ms))
        {
            let expired = active.take();
            drop(active);
            signal_shutdown(expired);
            return Ok(None);
        }
        Ok(active.as_ref().map(project_session))
    }

    pub(super) fn stop(&self) -> Result<bool, CaptureLanError> {
        let active = self
            .active
            .lock()
            .map_err(|_| CaptureLanError::Unavailable)?
            .take();
        let stopped = active.is_some();
        signal_shutdown(active);
        Ok(stopped)
    }

    pub(super) fn downgrade(&self) -> WeakCaptureLanSessionRegistry {
        WeakCaptureLanSessionRegistry {
            active: Arc::downgrade(&self.active),
        }
    }

    pub(super) fn remove_if_session(&self, session_id: &str) -> Result<bool, CaptureLanError> {
        let removed = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| CaptureLanError::Unavailable)?;
            if active
                .as_ref()
                .is_some_and(|active| active.session_id == session_id)
            {
                active.take()
            } else {
                None
            }
        };
        let found = removed.is_some();
        signal_shutdown(removed);
        Ok(found)
    }

    pub(super) fn shutdown_if_last_owner(&self) {
        if Arc::strong_count(&self.active) != 1 {
            return;
        }
        let active = self.active.lock().ok().and_then(|mut active| active.take());
        signal_shutdown(active);
    }
}

impl WeakCaptureLanSessionRegistry {
    pub(super) fn remove_if_session(&self, session_id: &str) {
        let Some(active) = self.active.upgrade() else {
            return;
        };
        let removed = active.lock().ok().and_then(|mut active| {
            if active
                .as_ref()
                .is_some_and(|active| active.session_id == session_id)
            {
                active.take()
            } else {
                None
            }
        });
        signal_shutdown(removed);
    }
}

fn project_session(active: &ActiveSession) -> CaptureLanSession {
    let activity = active.state.activity_snapshot();
    CaptureLanSession {
        session_id: active.session_id.clone(),
        batch_id: active.batch_id.clone(),
        qr_svg_data_url: active.qr_svg_data_url.clone(),
        selected_address: active.selected_address.clone(),
        expires_at_utc_ms: active.state.expires_at(&activity) as f64,
        received_item_count: activity.received_item_count,
        received_bytes: activity.received_bytes as f64,
    }
}

fn signal_shutdown(active: Option<ActiveSession>) {
    if let Some(active) = active {
        let _ = active.shutdown.send(true);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::Connection;
    use tokio::sync::Semaphore;

    use super::*;
    use crate::modules::capture_lan::{ABSOLUTE_TIMEOUT_MS, CaptureLanContext, SessionActivity};

    #[test]
    fn stale_server_cleanup_does_not_remove_the_replacement_session() {
        let sessions = CaptureLanSessionRegistry::default();
        let (old_state, old_shutdown, old_receiver) = server_state("old", 0, 0);
        install(&sessions, "old", old_state, old_shutdown);
        let weak_sessions = sessions.downgrade();

        sessions
            .ensure_startable(ABSOLUTE_TIMEOUT_MS)
            .expect("expired session cleanup");
        assert!(*old_receiver.borrow());

        let (new_state, new_shutdown, new_receiver) =
            server_state("replacement", ABSOLUTE_TIMEOUT_MS, ABSOLUTE_TIMEOUT_MS);
        install(&sessions, "replacement", new_state, new_shutdown);
        assert!(
            !sessions
                .remove_if_session("old")
                .expect("stale strong cleanup")
        );
        weak_sessions.remove_if_session("old");

        let status = sessions
            .status(ABSOLUTE_TIMEOUT_MS)
            .expect("replacement status")
            .expect("replacement session");
        assert_eq!(status.session_id, "replacement");
        assert!(!*new_receiver.borrow());
    }

    #[test]
    fn explicit_stop_signals_the_active_session() {
        let sessions = CaptureLanSessionRegistry::default();
        let (state, shutdown, receiver) = server_state("active", 0, 0);
        install(&sessions, "active", state, shutdown);
        let (other_state, other_shutdown, _other_receiver) = server_state("other", 0, 0);

        let duplicate = sessions.install(
            "other".to_owned(),
            "batch".to_owned(),
            "192.168.1.8".to_owned(),
            "data:image/svg+xml;base64,other".to_owned(),
            other_state,
            other_shutdown,
        );
        assert!(matches!(duplicate, Err(CaptureLanError::AlreadyActive)));
        assert!(sessions.stop().expect("first stop"));
        assert!(*receiver.borrow());
        assert!(!sessions.stop().expect("second stop"));
    }

    fn install(
        sessions: &CaptureLanSessionRegistry,
        session_id: &str,
        state: Arc<ServerState>,
        shutdown: watch::Sender<bool>,
    ) {
        sessions
            .install(
                session_id.to_owned(),
                "batch".to_owned(),
                "192.168.1.8".to_owned(),
                "data:image/svg+xml;base64,test".to_owned(),
                state,
                shutdown,
            )
            .expect("install session");
    }

    fn server_state(
        session_id: &str,
        started_at_utc_ms: i64,
        last_activity_utc_ms: i64,
    ) -> (Arc<ServerState>, watch::Sender<bool>, watch::Receiver<bool>) {
        let (shutdown, receiver) = watch::channel(false);
        let state = Arc::new(ServerState {
            session_id: session_id.to_owned(),
            context: CaptureLanContext {
                connection: Arc::new(Mutex::new(
                    Connection::open_in_memory().expect("connection"),
                )),
                blob_root: PathBuf::from("capture-blobs"),
                asset_key: [7; 32],
                account_id: "account".to_owned(),
                profile_id: "profile".to_owned(),
                batch_id: "batch".to_owned(),
                notifier: Arc::new(|_| {}),
            },
            public_origin: "http://192.168.1.8:1234".to_owned(),
            expected_host: "192.168.1.8:1234".to_owned(),
            token_hash: [3; 32],
            sequence_base: 0,
            started_at_utc_ms,
            activity: Mutex::new(SessionActivity {
                last_activity_utc_ms,
                received_item_count: 0,
                received_bytes: 0,
                next_source_sequence: 0,
            }),
            upload_slots: Arc::new(Semaphore::new(2)),
            shutdown: shutdown.clone(),
        });
        (state, shutdown, receiver)
    }
}
