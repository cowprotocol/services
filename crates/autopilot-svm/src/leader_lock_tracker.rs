use {database::leader_pg_lock::LeaderLock, observe::metrics};

/// Tracks the autopilot leader lock status across calls to
/// [`Self::try_acquire`].
pub enum LeaderLockTracker {
    /// No lock: the only running instance leads.
    Disabled,
    /// Several instances may run at once, and only the one holding the lock
    /// cuts auctions. The others keep syncing to the tip so they can take
    /// over at once.
    Enabled {
        /// Whether the instance held the lock after the last `try_acquire`.
        is_leader: bool,
        /// Whether the instance held the lock after the `try_acquire` before.
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

    /// Takes the lock, or checks that the held one is still alive. Called at
    /// the start of every cycle.
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

    /// Releases the lock if this instance holds it. Called once the loop
    /// stops.
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

    /// Whether the last `try_acquire` took the lock.
    fn just_stepped_up(&self) -> bool {
        matches!(
            self,
            Self::Enabled {
                is_leader: true,
                was_leader: false,
                ..
            }
        )
    }

    /// Whether this instance leads. Always true without a lock.
    pub fn is_leader(&self) -> bool {
        match self {
            Self::Enabled { is_leader, .. } => *is_leader,
            Self::Disabled => true,
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "leader_lock_tracker")]
struct Metrics {
    /// 1 after this instance stepped up as the leader, 0 after it stepped
    /// down.
    is_leader: prometheus::IntGauge,

    /// Errors taking or checking the leader lock.
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
