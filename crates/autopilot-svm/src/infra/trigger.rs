//! Chain-driven cycle trigger: wakes the loop once per new slot.

use {
    crate::{domain::cycle::SolanaCycle, run_loop::CycleTrigger},
    async_trait::async_trait,
    cow_solana_rpc::SolanaRPC,
    std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    },
};

/// How often the poller asks the node for the current slot. Half a slot, so
/// a new slot is typically observed within one poll of its arrival.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Yields each newly observed slot. A background task keeps the tip fresh
/// through the whole cycle, so the tip read after ranking reflects the time
/// the solvers spent and the submission deadline starts from now, not from
/// the cut.
pub struct SlotTrigger {
    tip: Arc<AtomicU64>,
    /// Minimum time between cycles. Zero yields one cycle per new slot.
    min_interval: Duration,
    /// When the last cycle fired, to hold `min_interval` before the next one.
    last_fired: Option<Instant>,
    /// The slot the last cycle was yielded for.
    last_yielded: u64,
}

impl SlotTrigger {
    /// Spawns the background slot poller. Must run inside a tokio runtime.
    pub fn new(rpc: SolanaRPC, min_interval: Duration) -> Self {
        let tip = Arc::new(AtomicU64::new(0));
        let poller = Arc::clone(&tip);
        tokio::spawn(async move {
            loop {
                match rpc.slot().await {
                    // The slot can regress across RPC nodes, the tip only
                    // moves forward.
                    Ok(slot) => {
                        poller.fetch_max(slot, Ordering::Relaxed);
                    }
                    Err(err) => tracing::warn!(?err, "failed to poll the slot"),
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
        Self {
            tip,
            min_interval,
            last_fired: None,
            last_yielded: 0,
        }
    }
}

#[async_trait]
impl CycleTrigger<SolanaCycle> for SlotTrigger {
    async fn next_cycle(&mut self) -> u64 {
        // Hold at least `min_interval` since the last cycle, then wait for
        // the poller to observe a new slot.
        if let Some(last) = self.last_fired {
            let since = last.elapsed();
            if since < self.min_interval {
                tokio::time::sleep(self.min_interval - since).await;
            }
        }
        loop {
            let slot = self.tip.load(Ordering::Relaxed);
            if slot > self.last_yielded {
                self.last_yielded = slot;
                self.last_fired = Some(Instant::now());
                return slot;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// The freshest polled slot, at most one poll interval old.
    fn current_tip(&self) -> u64 {
        self.tip.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest},
    };

    /// The cycle fires once the poller observes the slot, and the tip stays
    /// readable afterwards.
    #[tokio::test]
    async fn yields_the_polled_slot() {
        let rpc = SolanaRPC::new_mock_with_mocks(Mocks::from([(
            RpcRequest::GetSlot,
            serde_json::json!(42u64),
        )]));
        let mut trigger = SlotTrigger::new(rpc, Duration::ZERO);
        assert_eq!(trigger.next_cycle().await, 42);
        assert_eq!(trigger.current_tip(), 42);
    }
}
