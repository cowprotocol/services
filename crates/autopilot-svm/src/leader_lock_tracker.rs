#[cfg(test)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use {database::leader_pg_lock::LeaderLock, observe::metrics};

/// Tracks the autopilot leader lock status across calls to
/// [`Self::update`].
pub enum LeaderLockTracker {
    /// No lock: the only running instance leads.
    Disabled,
    /// Several instances may run at once, and only the one holding the lock
    /// cuts auctions. The others keep syncing to the tip so they can take
    /// over at once.
    Enabled {
        /// Whether the instance led at the last `update`.
        was_leader: bool,
        leader_lock: LeaderLock,
    },
    /// Leadership the test flips.
    #[cfg(test)]
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

    /// Logs and records leadership changes since the last call, and returns
    /// whether this instance leads. Called once per cycle, right before the
    /// leader-only work.
    pub fn update(&mut self) -> bool {
        let Self::Enabled {
            was_leader,
            leader_lock,
        } = self
        else {
            return self.is_leader();
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
        is_leader
    }

    /// Releases the lock. Called once the loop stops.
    pub async fn release(self) {
        if let Self::Enabled { leader_lock, .. } = self {
            if leader_lock.is_leader() {
                tracing::info!("Shutdown received, stepping down as the leader");
                Metrics::leader_step_down();
            }
            leader_lock.release().await;
        }
    }

    /// Whether this instance leads right now. Always true without a lock.
    pub fn is_leader(&self) -> bool {
        match self {
            Self::Enabled { leader_lock, .. } => leader_lock.is_leader(),
            Self::Disabled => true,
            #[cfg(test)]
            Self::Scripted(leading) => leading.load(Ordering::Acquire),
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "leader_lock_tracker")]
struct Metrics {
    /// 1 while this instance leads, 0 while it follows.
    is_leader: prometheus::IntGauge,

    /// Errors taking or keeping the leader lock.
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
