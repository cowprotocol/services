use {
    super::Postgres,
    crate::database::orders::OrderStoring,
    anyhow::{Context, Result},
    database::{
        auction::{self, Auction as DbAuction, AuctionId},
        fee_policies::{self, FeePolicy},
        order_events::{self, OrderEvent},
        order_execution::{self, OrderExecution as OrderExecutionRow},
        settlement_executions::{self, SettlementExecution as SettlementExecutionRow},
        solver_competition_v2::{self, OrderProposedSolution},
        trades::{self, TradesQueryRow},
    },
    futures::TryStreamExt,
    model::order::{Order, OrderUid},
};

pub struct DebugReport {
    pub order: Order,
    pub events: Vec<OrderEvent>,
    pub proposed_solutions: Vec<OrderProposedSolution>,
    pub auctions: Vec<DbAuction>,
    pub executions: Vec<OrderExecutionRow>,
    pub trades: Vec<TradesQueryRow>,
    pub settlement_executions: Vec<SettlementExecutionRow>,
    pub fee_policies: Vec<FeePolicy>,
}

impl Postgres {
    pub async fn fetch_debug_report(&self, uid: &OrderUid) -> Result<Option<DebugReport>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_debug_report"])
            .start_timer();

        let db_uid = database::byte_array::ByteArray(uid.0);
        let order = match self.single_order(uid).await? {
            Some(order) => order,
            None => return Ok(None),
        };

        let mut conn = self.pool.acquire().await?;

        let events = order_events::get_all(&mut conn, &db_uid).await?;
        let proposed_solutions =
            solver_competition_v2::find_solutions_for_order(&mut conn, &db_uid).await?;
        let executions = order_execution::read_by_order_uid(&mut conn, &db_uid).await?;
        let trades: Vec<TradesQueryRow> = trades::trades(&mut conn, None, Some(&db_uid), 0, 100)
            .into_inner()
            .try_collect()
            .await
            .context("failed to fetch trades")?;

        let auction_ids: Vec<AuctionId> =
            auction::fetch_auction_ids_by_order_uid(&mut conn, &db_uid).await?;

        let auctions = if auction_ids.is_empty() {
            vec![]
        } else {
            auction::fetch_multiple(&mut conn, &auction_ids).await?
        };
        let settlement_executions = if auction_ids.is_empty() {
            vec![]
        } else {
            settlement_executions::read_by_auction_ids(&mut conn, &auction_ids).await?
        };
        let fee_policies = fee_policies::fetch_by_order_uid(&mut conn, &db_uid).await?;

        Ok(Some(DebugReport {
            order,
            events,
            proposed_solutions,
            auctions,
            executions,
            trades,
            settlement_executions,
            fee_policies,
        }))
    }
}
