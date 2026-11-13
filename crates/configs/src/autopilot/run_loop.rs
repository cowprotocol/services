use {
    serde::Deserialize,
    std::{num::NonZeroUsize, time::Duration},
};

const fn default_max_delay() -> Duration {
    Duration::from_secs(2)
}

const fn default_max_winners_per_auction() -> NonZeroUsize {
    NonZeroUsize::new(20).unwrap()
}

const fn default_max_solutions_per_solver() -> NonZeroUsize {
    NonZeroUsize::new(3).unwrap()
}

const fn default_submission_deadline() -> u64 {
    5
}

const fn default_max_settlement_transaction_wait() -> Duration {
    Duration::from_mins(1)
}

const fn default_min_solve_time() -> Duration {
    Duration::from_secs(10)
}

const fn default_auction_delta_checkpoint_interval() -> Duration {
    Duration::from_mins(2)
}

const fn default_canonical_batch_cutoff() -> Duration {
    Duration::from_secs(5)
}

/// Configuration for the autopilot run loop timing.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RunLoopConfig {
    /// If a new run loop iteration would start more than this duration after
    /// the latest block was noticed, wait for the next block before continuing.
    #[serde(with = "humantime_serde", default = "default_max_delay")]
    pub max_delay: Duration,

    /// Maximum number of winners per auction. Each winner settles their
    /// winning orders concurrently.
    #[serde(default = "default_max_winners_per_auction")]
    pub max_winners_per_auction: NonZeroUsize,

    /// Maximum number of solutions a single solver may propose per auction.
    #[serde(default = "default_max_solutions_per_solver")]
    pub max_solutions_per_solver: NonZeroUsize,

    /// Enable leader lock in the database; the follower instance will not
    /// cut auctions.
    #[serde(default)]
    pub enable_leader_lock: bool,

    /// How often a full auction (checkpoint) is sent to drivers that opted
    /// into incremental auctions (`supports-auction-deltas`). The first
    /// auction that starts after this much time passed since the last
    /// checkpoint is sent in full; the auctions in between are sent as
    /// deltas relative to the previously sent auction.
    #[serde(
        with = "humantime_serde",
        default = "default_auction_delta_checkpoint_interval"
    )]
    pub auction_delta_checkpoint_interval: Duration,

    /// The maximum number of blocks to wait for a settlement to appear on
    /// chain.
    #[serde(default = "default_submission_deadline")]
    pub submission_deadline: u64,

    /// The amount of time that the autopilot waits looking for a settlement
    /// transaction onchain after the driver acknowledges the receipt of a
    /// settlement.
    #[serde(
        with = "humantime_serde",
        default = "default_max_settlement_transaction_wait"
    )]
    pub max_settlement_transaction_wait: Duration,

    /// Lower bound of time solvers have to compute solutions per auction.
    #[serde(
        with = "humantime_serde",
        rename = "solve-deadline",
        default = "default_min_solve_time"
    )]
    pub min_solve_time: Duration,

    /// If this is configured the `/solve` deadline will be picked
    /// to end shortly before a new block gets mined.
    /// This should not be configured on chains with variable block
    /// production rates (including test chains).
    #[serde(default)]
    pub sync_solve_deadline_to_blockchain: Option<SlotConfig>,

    /// Interval between canonical batches. Orders placed with
    /// `enableWaitForCow` only trade in canonical batches, which start at
    /// multiples of this interval since the Unix epoch. `None` disables
    /// canonical batches: wait-for-CoW orders trade like regular orders.
    #[serde(with = "humantime_serde", default)]
    pub canonical_batch_interval: Option<Duration>,

    /// Wait-for-CoW orders created less than this long before a canonical
    /// batch time join the batch after it, so that every order in a batch has
    /// been committed or indexed before the batch is built. Must be shorter
    /// than `canonical-batch-interval`.
    #[serde(with = "humantime_serde", default = "default_canonical_batch_cutoff")]
    pub canonical_batch_cutoff: Duration,
}

impl RunLoopConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        let Some(interval) = self.canonical_batch_interval else {
            return Ok(());
        };
        anyhow::ensure!(
            interval.subsec_nanos() == 0 && self.canonical_batch_cutoff.subsec_nanos() == 0,
            "canonical-batch-interval and canonical-batch-cutoff must be whole seconds"
        );
        // The orderbook reads the published interval as a 32-bit signed
        // integer.
        anyhow::ensure!(
            interval >= Duration::from_secs(1)
                && interval.as_secs() <= i32::MAX.unsigned_abs().into(),
            "canonical-batch-interval must be between 1 and i32::MAX seconds"
        );
        anyhow::ensure!(
            self.canonical_batch_cutoff < interval,
            "canonical-batch-cutoff must be shorter than canonical-batch-interval"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SlotConfig {
    /// How long each slot on a PoS chain is.
    #[serde(with = "humantime_serde")]
    pub slot_length: Duration,

    /// Minimum amount of time one has to submit a tx BEFORE the slot's end
    /// to still have it included in the next block.
    #[serde(with = "humantime_serde")]
    pub tx_propagation_latency: Duration,
}

impl Default for RunLoopConfig {
    fn default() -> Self {
        Self {
            max_delay: default_max_delay(),
            max_winners_per_auction: default_max_winners_per_auction(),
            max_solutions_per_solver: default_max_solutions_per_solver(),
            enable_leader_lock: false,
            auction_delta_checkpoint_interval: default_auction_delta_checkpoint_interval(),
            submission_deadline: default_submission_deadline(),
            max_settlement_transaction_wait: default_max_settlement_transaction_wait(),
            min_solve_time: default_min_solve_time(),
            sync_solve_deadline_to_blockchain: None,
            canonical_batch_interval: None,
            canonical_batch_cutoff: default_canonical_batch_cutoff(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for RunLoopConfig {
    fn test_default() -> Self {
        Self {
            max_delay: Duration::from_millis(100),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: RunLoopConfig = toml::from_str("").unwrap();
        assert_eq!(config.max_delay, Duration::from_secs(2));
        assert_eq!(
            config.auction_delta_checkpoint_interval,
            Duration::from_mins(2)
        );
        assert_eq!(config.canonical_batch_interval, None);
        assert_eq!(config.canonical_batch_cutoff, Duration::from_secs(5));
        config.validate().unwrap();
    }

    #[test]
    fn validate_canonical_batches() {
        let config = |interval: Option<Duration>, cutoff: Duration| RunLoopConfig {
            canonical_batch_interval: interval,
            canonical_batch_cutoff: cutoff,
            ..Default::default()
        };
        let secs = Duration::from_secs;

        // Off: the cutoff isn't checked.
        config(None, secs(120)).validate().unwrap();

        config(Some(secs(60)), secs(5)).validate().unwrap();
        config(Some(secs(60)), secs(0)).validate().unwrap();
        config(Some(secs(1)), secs(0)).validate().unwrap();

        // The cutoff must be shorter than the interval.
        assert!(config(Some(secs(60)), secs(60)).validate().is_err());
        // No zero interval.
        assert!(config(Some(secs(0)), secs(0)).validate().is_err());
        // Whole seconds only.
        assert!(
            config(Some(Duration::from_millis(1500)), secs(0))
                .validate()
                .is_err()
        );
        assert!(
            config(Some(secs(60)), Duration::from_millis(500))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        max-delay = "5s"
        auction-delta-checkpoint-interval = "30s"
        canonical-batch-interval = "1m"
        canonical-batch-cutoff = "3s"
        [sync-solve-deadline-to-blockchain]
            slot-length = "12s"
            tx-propagation-latency = "2s"
        "#;
        let config: RunLoopConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(
            config.auction_delta_checkpoint_interval,
            Duration::from_secs(30)
        );
        assert_eq!(
            config.canonical_batch_interval,
            Some(Duration::from_mins(1))
        );
        assert_eq!(config.canonical_batch_cutoff, Duration::from_secs(3));
        assert_eq!(
            config
                .sync_solve_deadline_to_blockchain
                .as_ref()
                .unwrap()
                .slot_length,
            Duration::from_secs(12)
        );
        assert_eq!(
            config
                .sync_solve_deadline_to_blockchain
                .as_ref()
                .unwrap()
                .tx_propagation_latency,
            Duration::from_secs(2)
        );
    }
}
