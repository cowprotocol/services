//! Competition bookkeeping. Auction progress is written to
//! `solana.order_events`, everything else is logged only: there are no
//! competition tables (auction snapshots, proposed executions) to write.
//!
//! TODO: persist the competition outcome once the tables exist. The
//! settlement attribution (`solana.settlements.solution_uid`) depends on
//! the persisted ranking.

use {
    crate::{
        domain::{auction::Auction, cycle::Ranking},
        infra::{observation::SettlementWindows, order_events},
        run_loop::SettlementObserver,
    },
    async_trait::async_trait,
    chain_types::solana::IntentHash,
    database::order_events::OrderEventLabel,
    sqlx::PgPool,
    std::collections::HashSet,
};

/// Writes order events, logs the competition phases, and drives the
/// settlement-timeout check off the per-cycle tip.
pub struct CompetitionObserver {
    pool: PgPool,
    windows: SettlementWindows,
}

impl CompetitionObserver {
    pub fn new(pool: PgPool, windows: SettlementWindows) -> Self {
        Self { pool, windows }
    }

    /// Store the events without blocking the cycle: a lost event degrades the
    /// status endpoint, never the competition.
    fn store_events(&self, uids: Vec<IntentHash>, label: OrderEventLabel) {
        let pool = self.pool.clone();
        tokio::spawn(async move {
            if let Err(err) = order_events::store(&pool, uids, label).await {
                tracing::error!(?err, ?label, "failed to store order events");
            }
        });
    }
}

#[async_trait]
impl SettlementObserver<crate::domain::cycle::SolanaCycle> for CompetitionObserver {
    fn on_orders_ready(&self, auction: &Auction) {
        tracing::debug!(orders = auction.orders.len(), "auction entered competition");
        self.store_events(
            auction.orders.iter().map(|order| order.uid).collect(),
            OrderEventLabel::Ready,
        );
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
        self.store_events(executing.into_iter().collect(), OrderEventLabel::Executing);
        self.store_events(
            considered.into_iter().collect(),
            OrderEventLabel::Considered,
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
