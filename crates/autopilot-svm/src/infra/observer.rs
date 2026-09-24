//! Competition bookkeeping: auction progress in `solana.order_events`, the
//! ranked outcome in the competition tables.

use {
    crate::{
        domain::{auction::Auction, cycle::Ranking},
        infra::{db, observation::SettlementWindows, order_events},
        run_loop::SettlementObserver,
    },
    async_trait::async_trait,
    bigdecimal::BigDecimal,
    chain_types::solana::IntentHash,
    database::{byte_array::ByteArray, solana::OrderEventLabel},
    sqlx::PgPool,
    std::{collections::HashSet, sync::Mutex},
    winner_selection::state::RankedItem,
};

/// Writes order events, logs the competition phases, and drives the
/// settlement-timeout check off the per-cycle tip.
pub struct CompetitionObserver {
    pool: PgPool,
    windows: SettlementWindows,
    /// The previous auction's order uids, diffed against the current ones so
    /// the logs carry the change instead of repeating the full list.
    previous_orders: Mutex<HashSet<IntentHash>>,
}

impl CompetitionObserver {
    pub fn new(pool: PgPool, windows: SettlementWindows) -> Self {
        Self {
            pool,
            windows,
            previous_orders: Mutex::default(),
        }
    }

    /// Store the events without blocking the cycle: a lost event degrades the
    /// status endpoint, never the competition.
    fn store_events(&self, uids: Vec<IntentHash>, label: OrderEventLabel) {
        order_events::store_detached(self.pool.clone(), uids, label);
    }
}

#[async_trait]
impl SettlementObserver<crate::domain::cycle::SolanaCycle> for CompetitionObserver {
    fn on_orders_ready(&self, auction: &Auction) {
        tracing::info!(
            auction_id = auction.id,
            orders = auction.orders.len(),
            "solving"
        );
        let current: HashSet<IntentHash> = auction.orders.iter().map(|order| order.uid).collect();
        {
            let mut previous = self.previous_orders.lock().unwrap();
            let added: Vec<String> = current
                .difference(&previous)
                .map(ToString::to_string)
                .collect();
            tracing::debug!(auction_id = auction.id, ?added, "new orders in auction");
            let removed: Vec<String> = previous
                .difference(&current)
                .map(ToString::to_string)
                .collect();
            tracing::debug!(
                auction_id = auction.id,
                ?removed,
                "orders no longer in auction"
            );
            *previous = current.clone();
        }
        self.store_events(current.into_iter().collect(), OrderEventLabel::Ready);
    }

    async fn persist_competition_ranking(
        &self,
        auction: &Auction,
        tip: &u64,
        ranking: &Ranking,
        deadline: u64,
    ) -> anyhow::Result<()> {
        // Best effort: the expiry bookkeeping only touches previously
        // dispatched windows and must not block the current dispatch.
        if let Err(err) = self.windows.expire_past_deadline(*tip).await {
            tracing::error!(?err, "failed to flag expired settlement windows");
        }
        let solutions = ranking
            .enumerated()
            .map(|(uid, solution)| db::ProposedSolution {
                uid,
                id: i64::try_from(solution.id()).unwrap_or(i64::MAX),
                solver: ByteArray(solution.solver().0),
                is_winner: solution.is_winner(),
                filtered_out: solution.is_filtered_out(),
                score: BigDecimal::from(solution.score()),
                trades: solution
                    .orders()
                    .iter()
                    .map(|order| db::ProposedTrade {
                        order_uid: ByteArray(order.uid.0),
                        executed_sell: BigDecimal::from(order.executed_sell),
                        executed_buy: BigDecimal::from(order.executed_buy),
                    })
                    .collect(),
            })
            .collect();
        let (price_tokens, price_values) = auction
            .native_prices
            .iter()
            .map(|(token, price)| (token.0.to_vec(), BigDecimal::from(*price)))
            .unzip();
        let competition = db::Competition {
            auction_id: auction.id,
            tip_slot: i64::try_from(*tip).unwrap_or(i64::MAX),
            deadline_slot: i64::try_from(deadline).unwrap_or(i64::MAX),
            order_uids: auction
                .orders
                .iter()
                .map(|order| order.uid.0.to_vec())
                .collect(),
            price_tokens,
            price_values,
            solutions,
            reference_scores: ranking
                .reference_scores
                .iter()
                .map(|(solver, score)| db::ReferenceScore {
                    solver: ByteArray(solver.0),
                    score: BigDecimal::from(*score),
                })
                .collect(),
        };
        db::persist_competition(&self.pool, &competition).await?;
        tracing::info!(
            auction_id = auction.id,
            tip,
            deadline,
            winners = ranking.inner.winners().count(),
            ranked = ranking.inner.ranked.len(),
            filtered_out = ranking.inner.filtered_out.len(),
            "competition persisted"
        );
        Ok(())
    }

    fn on_orders_matched(&self, executing: HashSet<IntentHash>, considered: HashSet<IntentHash>) {
        tracing::debug!(
            executing = ?executing.iter().map(ToString::to_string).collect::<Vec<_>>(),
            considered = ?considered.iter().map(ToString::to_string).collect::<Vec<_>>(),
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
