#[cfg(feature = "test-util")]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use {crate::leader_pg_lock::LeaderLock, observe::metrics};

/// Tracks the autopilot leader lock status.
pub enum LeaderLockTracker {
    /// Leader lock mechanism is disabled.
    /// Only one autopilot instance should be running at all times.
    Disabled,
    /// Leader lock mechanism is enabled.
    /// This allows for multiple autopilots to run simultaneously with only one
    /// driving the auctions forward.
    /// The autopilot followers (not holding the lock) will keep their caches
    /// warm. It facilitates zero downtime deployments.
    Enabled {
        /// Whether the instance was the leader at the last call to
        /// `update()`
        was_leader: bool,
        leader_lock: LeaderLock,
    },
    /// Leadership the test flips.
    #[cfg(feature = "test-util")]
    Scripted(Arc<AtomicBool>),
}

impl LeaderLockTracker {
    pub fn new(leader_lock: Option<LeaderLock>) -> Self {
        match leader_lock {
            Some(leader_lock) => {
                // followers export 0, so a missing leader shows up
                Metrics::leader_step_down();
                Self::Enabled {
                    was_leader: false,
                    leader_lock,
                }
            }
            None => Self::Disabled,
        }
    }

    /// Logs and records leadership changes since the last call. Called once
    /// per iteration, right before the leader-only work.
    pub fn update(&mut self) {
        let Self::Enabled {
            was_leader,
            leader_lock,
        } = self
        else {
            return;
        };

        Metrics::leader_lock_errors(leader_lock.take_errors());
        let is_leader = leader_lock.is_leader();
        match (*was_leader, is_leader) {
            (false, true) => {
                tracing::info!("Stepped up as a leader");
                Metrics::leader_step_up();
            }
            (true, false) => {
                tracing::warn!("Lost the leader lock, stepping down");
                Metrics::leader_step_down();
            }
            _ => {}
        }
        *was_leader = is_leader;
    }

    /// Releases the leader lock
    /// Should be called after breaking out of run loop (for example: due to
    /// shutdown)
    pub async fn release(self) {
        if let Self::Enabled { leader_lock, .. } = self {
            if leader_lock.is_leader() {
                tracing::info!("Shutdown received, stepping down as the leader");
                Metrics::leader_step_down();
            }
            leader_lock.release().await;
        }
    }

    /// Returns true while the leader lock is held. Check it right before
    /// every leader-only action, the lock can be lost at any time
    /// If the feature is disabled, always returns true
    pub fn is_leader(&self) -> bool {
        match self {
            Self::Enabled { leader_lock, .. } => leader_lock.is_leader(),
            Self::Disabled => true,
            #[cfg(feature = "test-util")]
            Self::Scripted(leading) => leading.load(Ordering::Acquire),
        }
    }
}
#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "leader_lock_tracker")]
struct Metrics {
    /// Tracks the current leader status
    /// 1 - is currently autopilot leader
    /// 0 - is currently autopilot follower
    is_leader: prometheus::IntGauge,

    /// Tracks the count of errors taking or keeping the leader lock
    leader_lock_error: prometheus::IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(metrics::get_storage_registry()).unwrap()
    }

    fn leader_step_up() {
        Self::get().is_leader.set(1)
    }

    fn leader_step_down() {
        Self::get().is_leader.set(0)
    }

    fn leader_lock_errors(count: u64) {
        Self::get().leader_lock_error.inc_by(count)
    }
}
