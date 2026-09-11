//! Chain-driven cycle trigger: wakes the loop once per new slot.

use {
    crate::{domain::cycle::SolanaCycle, run_loop::CycleTrigger},
    async_trait::async_trait,
    cow_solana_rpc::SolanaRPC,
    std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    },
};

/// How often the trigger asks the node for the current slot. Half a slot, so
/// a new slot is typically observed within one poll of its arrival.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Polls the node for the slot and yields each newly observed one.
pub struct SlotTrigger {
    rpc: SolanaRPC,
    tip: AtomicU64,
    /// Minimum time between cycles. Zero yields one cycle per new slot.
    min_interval: Duration,
    /// When the last cycle fired, to hold `min_interval` before the next one.
    last_fired: Option<Instant>,
}

impl SlotTrigger {
    pub fn new(rpc: SolanaRPC, min_interval: Duration) -> Self {
        Self {
            rpc,
            tip: AtomicU64::new(0),
            min_interval,
            last_fired: None,
        }
    }
}

#[async_trait]
impl CycleTrigger<SolanaCycle> for SlotTrigger {
    async fn next_cycle(&mut self) -> u64 {
        // Hold at least `min_interval` since the last cycle, then poll for the
        // freshest slot.
        if let Some(last) = self.last_fired {
            let since = last.elapsed();
            if since < self.min_interval {
                tokio::time::sleep(self.min_interval - since).await;
            }
        }
        loop {
            match self.rpc.slot().await {
                Ok(slot) => {
                    let previous = self.tip.fetch_max(slot, Ordering::Relaxed);
                    if slot > previous {
                        self.last_fired = Some(Instant::now());
                        return slot;
                    }
                }
                Err(err) => tracing::warn!(?err, "failed to poll the slot"),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    // TODO: poll in the background so the tip observed after ranking is
    // fresher than the cut tip once the submission deadline is enforced.
    fn current_tip(&self) -> u64 {
        self.tip.load(Ordering::Relaxed)
    }
}
