use {
    super::Postgres,
    crate::database::orders::OrderStoring,
    alloy::primitives::Address,
    anyhow::{Context, Result},
    database::{
        auction::{self, Auction as DbAuction, AuctionId},
        byte_array::ByteArray,
        fee_policies::{self, FeePolicy as DbFeePolicy, FeePolicyKind as DbFeePolicyKind},
        order_events,
        order_execution::{self, OrderExecution as DbOrderExecution},
        settlement_executions::{self, SettlementExecution as DbSettlementExecution},
        solver_competition_v2::{self, OrderProposedSolution as DbProposedSolution},
        trades::{self, TradesQueryRow as DbTradesQueryRow},
    },
    futures::TryStreamExt,
    model::order::{Order, OrderUid},
    serde::Serialize,
    std::collections::HashMap,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugReport {
    pub order_uid: String,
    pub order: Order,
    pub events: Vec<Event>,
    pub auctions: Vec<Auction>,
    pub trades: Vec<Trade>,
}

impl Postgres {
    pub async fn fetch_debug_report(&self, uid: &OrderUid) -> Result<Option<DebugReport>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_debug_report"])
            .start_timer();

        let db_uid = ByteArray(uid.0);
        let order = match self.single_order(uid).await? {
            Some(order) => order,
            None => return Ok(None),
        };

        let mut conn = self.pool.acquire().await?;

        let events: Vec<Event> = order_events::get_all(&mut conn, &db_uid)
            .await?
            .into_iter()
            .map(|e| Event {
                label: format!("{:?}", e.label).to_lowercase(),
                timestamp: e.timestamp.to_rfc3339(),
            })
            .collect();

        let proposed_solutions =
            solver_competition_v2::find_solutions_for_order(&mut conn, &db_uid).await?;
        let executions = order_execution::read_by_order_uid(&mut conn, &db_uid).await?;
        let trades: Vec<DbTradesQueryRow> = trades::trades(&mut conn, None, Some(&db_uid), 0, 100)
            .into_inner()
            .try_collect()
            .await
            .context("failed to fetch trades")?;

        let auction_ids: Vec<AuctionId> =
            auction::fetch_auction_ids_by_order_uid(&mut conn, &db_uid).await?;

        let db_auctions = if auction_ids.is_empty() {
            vec![]
        } else {
            auction::fetch_multiple(&mut conn, &auction_ids).await?
        };
        let settlement_execs = if auction_ids.is_empty() {
            vec![]
        } else {
            settlement_executions::read_by_auction_ids(&mut conn, &auction_ids).await?
        };
        let fee_policies = fee_policies::fetch_by_order_uid(&mut conn, &db_uid).await?;

        let sell_token = order.data.sell_token;
        let buy_token = order.data.buy_token;
        let auctions = build_auctions(
            db_auctions,
            proposed_solutions,
            executions,
            settlement_execs,
            fee_policies,
            sell_token,
            buy_token,
        );

        Ok(Some(DebugReport {
            order_uid: uid.to_string(),
            order,
            events,
            auctions,
            trades: trades.into_iter().map(Trade::from).collect(),
        }))
    }
}

fn build_auctions(
    db_auctions: Vec<DbAuction>,
    db_solutions: Vec<DbProposedSolution>,
    db_executions: Vec<DbOrderExecution>,
    db_settlements: Vec<DbSettlementExecution>,
    db_fees: Vec<DbFeePolicy>,
    sell_token: Address,
    buy_token: Address,
) -> Vec<Auction> {
    let sell = ByteArray(sell_token.0.0);
    let buy = ByteArray(buy_token.0.0);

    let mut solutions_by_auction: HashMap<i64, Vec<ProposedSolution>> = HashMap::new();
    for s in db_solutions {
        solutions_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(ProposedSolution::from(s));
    }

    let mut executions_by_auction: HashMap<i64, Vec<Execution>> = HashMap::new();
    for e in db_executions {
        executions_by_auction
            .entry(e.auction_id)
            .or_default()
            .push(Execution::from(e));
    }

    let mut settlements_by_auction: HashMap<i64, Vec<SettlementAttempt>> = HashMap::new();
    for s in db_settlements {
        settlements_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(SettlementAttempt::from(s));
    }

    let mut fees_by_auction: HashMap<i64, Vec<FeePolicy>> = HashMap::new();
    for f in db_fees {
        fees_by_auction
            .entry(f.auction_id)
            .or_default()
            .push(FeePolicy::from(f));
    }

    let mut auctions: Vec<Auction> = db_auctions
        .into_iter()
        .map(|a| {
            let native_prices: HashMap<String, String> = a
                .price_tokens
                .iter()
                .zip(&a.price_values)
                .filter(|(token, _)| **token == sell || **token == buy)
                .map(|(token, price)| (token.to_string(), price.to_string()))
                .collect();
            let id = a.id;
            Auction {
                id,
                block: a.block,
                deadline: a.deadline,
                native_prices,
                proposed_solutions: solutions_by_auction.remove(&id).unwrap_or_default(),
                executions: executions_by_auction.remove(&id).unwrap_or_default(),
                settlement_attempts: settlements_by_auction.remove(&id).unwrap_or_default(),
                fee_policies: fees_by_auction.remove(&id).unwrap_or_default(),
            }
        })
        .collect();

    auctions.sort_by_key(|a| a.id);
    auctions
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub label: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: i64,
    pub block: i64,
    pub deadline: i64,
    pub native_prices: HashMap<String, String>,
    pub proposed_solutions: Vec<ProposedSolution>,
    pub executions: Vec<Execution>,
    pub settlement_attempts: Vec<SettlementAttempt>,
    pub fee_policies: Vec<FeePolicy>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedSolution {
    pub solution_uid: i64,
    pub ranking: i64,
    pub solver: String,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: String,
    pub executed_sell: String,
    pub executed_buy: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub executed_fee: String,
    pub executed_fee_token: String,
    pub block_number: i64,
    pub protocol_fees: Vec<ProtocolFee>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolFee {
    pub token: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub block_number: i64,
    pub log_index: i64,
    pub buy_amount: String,
    pub sell_amount: String,
    pub sell_amount_before_fees: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auction_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementAttempt {
    pub solver: String,
    pub solution_uid: i64,
    pub start_timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_timestamp: Option<String>,
    pub start_block: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_block: Option<i64>,
    pub deadline_block: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeePolicy {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_max_volume_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_improvement_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_improvement_max_volume_factor: Option<f64>,
}

impl From<DbProposedSolution> for ProposedSolution {
    fn from(s: DbProposedSolution) -> Self {
        Self {
            solution_uid: s.solution_uid,
            ranking: s.ranking,
            solver: Address::from(s.solver.0).to_string(),
            is_winner: s.is_winner,
            filtered_out: s.filtered_out,
            score: s.score.to_string(),
            executed_sell: s.executed_sell.to_string(),
            executed_buy: s.executed_buy.to_string(),
        }
    }
}

impl From<DbOrderExecution> for Execution {
    fn from(e: DbOrderExecution) -> Self {
        let protocol_fees = e
            .protocol_fee_tokens
            .iter()
            .zip(e.protocol_fee_amounts.iter())
            .map(|(token, amount)| ProtocolFee {
                token: Address::from(token.0).to_string(),
                amount: amount.to_string(),
            })
            .collect();
        Self {
            executed_fee: e.executed_fee.to_string(),
            executed_fee_token: Address::from(e.executed_fee_token.0).to_string(),
            block_number: e.block_number,
            protocol_fees,
        }
    }
}

impl From<DbTradesQueryRow> for Trade {
    fn from(t: DbTradesQueryRow) -> Self {
        Self {
            block_number: t.block_number,
            log_index: t.log_index,
            buy_amount: t.buy_amount.to_string(),
            sell_amount: t.sell_amount.to_string(),
            sell_amount_before_fees: t.sell_amount_before_fees.to_string(),
            tx_hash: t.tx_hash.map(|h| h.to_string()),
            auction_id: t.auction_id,
        }
    }
}

impl From<DbSettlementExecution> for SettlementAttempt {
    fn from(s: DbSettlementExecution) -> Self {
        Self {
            solver: Address::from(s.solver.0).to_string(),
            solution_uid: s.solution_uid,
            start_timestamp: s.start_timestamp.to_rfc3339(),
            end_timestamp: s.end_timestamp.map(|t| t.to_rfc3339()),
            start_block: s.start_block,
            end_block: s.end_block,
            deadline_block: s.deadline_block,
            outcome: s.outcome,
        }
    }
}

impl From<DbFeePolicy> for FeePolicy {
    fn from(f: DbFeePolicy) -> Self {
        Self {
            kind: match f.kind {
                DbFeePolicyKind::Surplus => "surplus",
                DbFeePolicyKind::Volume => "volume",
                DbFeePolicyKind::PriceImprovement => "priceImprovement",
            }
            .to_string(),
            surplus_factor: f.surplus_factor,
            surplus_max_volume_factor: f.surplus_max_volume_factor,
            volume_factor: f.volume_factor,
            price_improvement_factor: f.price_improvement_factor,
            price_improvement_max_volume_factor: f.price_improvement_max_volume_factor,
        }
    }
}
