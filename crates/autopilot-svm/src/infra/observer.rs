//! Competition bookkeeping. Outcomes are logged only: there are no
//! competition tables (auction snapshots, proposed executions, order
//! events) to write.
//!
//! TODO: persist the competition outcome once the tables exist. The
//! settlement attribution (`solana.settlements.solution_uid`) depends on
//! the persisted ranking.

use {
    crate::{
        domain::{auction::Auction, cycle::Ranking},
        infra::observation::SettlementWindows,
        run_loop::SettlementObserver,
    },
    async_trait::async_trait,
    chain_types::solana::IntentHash,
    std::collections::HashSet,
};

/// Logs the competition phases and drives the settlement-timeout check off
/// the per-cycle tip.
pub struct LogObserver {
    windows: SettlementWindows,
}

impl LogObserver {
    pub fn new(windows: SettlementWindows) -> Self {
        Self { windows }
    }
}

#[async_trait]
impl SettlementObserver<crate::domain::cycle::SolanaCycle> for LogObserver {
    fn on_orders_ready(&self, auction: &Auction) {
        tracing::debug!(orders = auction.orders.len(), "auction entered competition");
    }

    async fn persist_competition_ranking(
        &self,
        _auction: &Auction,
        tip: &u64,
        ranking: &Ranking,
        deadline: u64,
    ) -> anyhow::Result<()> {
        // Best effort: the expiry bookkeeping only touches previously
        // dispatched windows and must not block the current dispatch.
        if let Err(err) = self.windows.expire_past_deadline(*tip).await {
            tracing::error!(?err, "failed to flag expired settlement windows");
        }
        tracing::info!(
            tip,
            deadline,
            winners = ranking.inner.winners().count(),
            ranked = ranking.inner.ranked.len(),
            filtered_out = ranking.inner.filtered_out.len(),
            "competition ranked"
        );
        Ok(())
    }

    fn on_orders_matched(&self, executing: HashSet<IntentHash>, considered: HashSet<IntentHash>) {
        tracing::debug!(
            executing = executing.len(),
            considered = considered.len(),
            "orders matched"
        );
    }

    fn on_competition_ended(&self, auction: &Auction, ranking: &Ranking) {
        tracing::debug!(
            auction_id = auction.id,
            winners = ranking.inner.winners().count(),
            "competition ended"
        );
    }
}
