//! Chain-driven cycle trigger: wakes the loop once per new slot.

use {
    crate::{domain::cycle::SolanaCycle, run_loop::CycleTrigger},
    async_trait::async_trait,
    cow_solana_rpc::SolanaRPC,
    std::time::{Duration, Instant},
    tokio::sync::watch,
};

/// How often the poller asks the node for the current slot. Half a slot, so
/// a new slot is typically observed within one poll of its arrival.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Yields each newly observed slot. A background task keeps the tip fresh
/// through the whole cycle, so the tip read after ranking reflects the time
/// the solvers spent and the submission deadline starts from now, not from
/// the cut.
pub struct SlotTrigger {
    tip: watch::Receiver<u64>,
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
        let (sender, tip) = watch::channel(0);
        tokio::spawn(async move {
            // A fixed cadence: a slow RPC answer delays one poll, it does
            // not shift every following one.
            let mut poll = tokio::time::interval(POLL_INTERVAL);
            poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                poll.tick().await;
                match rpc.slot().await {
                    // The slot can regress across RPC nodes, the tip only
                    // moves forward.
                    Ok(slot) => {
                        sender.send_if_modified(|tip| {
                            let advanced = slot > *tip;
                            if advanced {
                                *tip = slot;
                            }
                            advanced
                        });
                    }
                    Err(err) => tracing::warn!(?err, "failed to poll the slot"),
                }
            }
        });
        Self {
            tip,
            min_interval,
            last_fired: None,
            last_yielded: 0,
        }
    }

    /// The poller's tip, for components that wait on slots outside the cycle.
    pub fn tip(&self) -> watch::Receiver<u64> {
        self.tip.clone()
    }
}

#[async_trait]
impl CycleTrigger<SolanaCycle> for SlotTrigger {
    async fn next_cycle(&mut self) -> u64 {
        // Hold at least `min_interval` since the last cycle, then wait for
        // the poller to observe a new slot. The watch wakes this on every
        // advance, so the yield adds no polling delay of its own.
        if let Some(last) = self.last_fired {
            let since = last.elapsed();
            if since < self.min_interval {
                tokio::time::sleep(self.min_interval - since).await;
            }
        }
        let last_yielded = self.last_yielded;
        let slot = *self
            .tip
            .wait_for(|slot| *slot > last_yielded)
            .await
            .expect("the slot poller never stops");
        self.last_yielded = slot;
        self.last_fired = Some(Instant::now());
        slot
    }

    /// The freshest polled slot, at most one poll interval old.
    fn current_tip(&self) -> u64 {
        *self.tip.borrow()
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
