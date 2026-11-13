//! Leader election between the instances of a service, so that only one of
//! them does the leader-only work.

mod leader_lock_tracker;
mod leader_pg_lock;

pub use {leader_lock_tracker::LeaderLockTracker, leader_pg_lock::LeaderLock};
