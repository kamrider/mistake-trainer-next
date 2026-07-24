use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct SyncCoordinator {
    running: Arc<AtomicBool>,
}

impl SyncCoordinator {
    pub fn try_begin(&self) -> Option<SyncPermit> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| SyncPermit {
                running: Arc::clone(&self.running),
            })
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

pub struct SyncPermit {
    running: Arc<AtomicBool>,
}

impl Drop for SyncPermit {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::SyncCoordinator;

    #[test]
    fn only_one_sync_permit_can_exist() {
        let coordinator = SyncCoordinator::default();
        let permit = coordinator.try_begin().expect("first permit");

        assert!(coordinator.is_running());
        assert!(coordinator.try_begin().is_none());

        drop(permit);
        assert!(!coordinator.is_running());
    }

    #[test]
    fn cloned_coordinators_share_the_same_permit() {
        let coordinator = SyncCoordinator::default();
        let clone = coordinator.clone();
        let _permit = clone.try_begin().expect("shared permit");

        assert!(coordinator.try_begin().is_none());
    }
}
