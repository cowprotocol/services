//! Leader election between the instances of a service, so that only one of
//! them does the leader-only work.

mod leader_lock_tracker;
mod leader_pg_lock;
#[cfg(feature = "test-util")]
pub mod test_util;

pub use {leader_lock_tracker::LeaderLockTracker, leader_pg_lock::LeaderPgLock};

/// A lock that at most one instance holds at a time.
#[async_trait::async_trait]
pub trait LeaderLock: Send + Sync {
    /// Whether this instance may act as the leader right now. The lock can
    /// be lost at any time, so check it right before every leader-only action.
    fn is_leader(&self) -> bool;

    /// Returns how many attempts to take or keep the lock failed since the
    /// last call, and resets the count.
    fn take_errors(&self) -> u64;

    /// Frees the lock, so that another instance can take it.
    async fn release(self: Box<Self>);
}
