use {database::leader_pg_lock::LeaderLock, observe::metrics};

/// Tracks the autopilot leader lock status.
/// Leader lock status is tracked based on calls to try_acquire()
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
        /// Whether the current instance is the leader since the last call to
        /// try_acquire()
        is_leader: bool,
        /// Whether the instance was the leader since the last call to
        /// try_acquire()
        was_leader: bool,
        leader_lock: LeaderLock,
    },
}

impl LeaderLockTracker {
    pub fn new(leader_lock: Option<LeaderLock>) -> Self {
        match leader_lock {
            Some(leader_lock) => Self::Enabled {
                is_leader: false,
                was_leader: false,
                leader_lock,
            },
            None => Self::Disabled,
        }
    }

    /// Tries to acquire the leader lock if it's enabled
    /// If not, does nothing
    /// Should be called at the beginning of every run loop iteration
    pub async fn try_acquire(&mut self) {
        let Self::Enabled {
            is_leader,
            was_leader,
            leader_lock,
        } = self
        else {
            return;
        };

        *was_leader = *is_leader;

        *is_leader = leader_lock.try_acquire().await.unwrap_or_else(|err| {
            tracing::error!(?err, "failed to become leader");
            Metrics::leader_lock_error();
            false
        });

        if self.just_stepped_up() {
            tracing::info!("Stepped up as a leader");
            Metrics::leader_step_up();
        }
    }

    /// Releases the leader lock if it was held
    /// Should be called after breaking out of run loop (for example: due to
    /// shutdown)
    pub async fn release(self) {
        if let Self::Enabled {
            mut leader_lock,
            is_leader: true,
            ..
        } = self
        {
            tracing::info!("Shutdown received, stepping down as the leader");
            leader_lock.release().await;
            Metrics::leader_step_down();
        }
    }

    /// Returns true if the last try_acquire call resulted in acquiring the
    /// leader lock If the feature is disabled, always returns false
    pub fn just_stepped_up(&self) -> bool {
        matches!(
            self,
            Self::Enabled {
                is_leader: true,
                was_leader: false,
                ..
            }
        )
    }

    /// Returns true if the leader lock is being held
    /// If the feature is disabled, always returns true
    pub fn is_leader(&self) -> bool {
        match self {
            Self::Enabled { is_leader, .. } => *is_leader,
            _ => true,
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

    /// Trackes the count of errors acquiring leader lock (should never happen)
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

    fn leader_lock_error() {
        Self::get().leader_lock_error.inc()
    }
}
