use {
    crate::LeaderLock,
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A lock whose leadership the test flips through the shared flag.
pub struct ScriptedLeaderLock(pub Arc<AtomicBool>);

#[async_trait::async_trait]
impl LeaderLock for ScriptedLeaderLock {
    fn is_leader(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn take_errors(&self) -> u64 {
        0
    }

    async fn release(self: Box<Self>) {}
}
