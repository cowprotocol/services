use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Json, Response},
    },
    database::{
        byte_array::ByteArray,
        fee_policies::{FeePolicy, FeePolicyKind},
        order_events::OrderEvent,
        order_execution::ExecutionRow as OrderExecutionRow,
        settlement_executions::ExecutionRow as SettlementExecutionRow,
        solver_competition_v2::OrderProposedSolution,
        trades::TradesQueryRow,
    },
    model::order::{Order, OrderUid},
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
};

pub async fn debug_order_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    headers: HeaderMap,
) -> Response {
    if state.debug_route_auth_tokens.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "debug endpoint is not enabled"),
        )
            .into_response();
    }

    let token_name = match authenticate(&headers, &state.debug_route_auth_tokens) {
        Some(name) => name,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                super::error("Unauthorized", "invalid or missing x-auth-token"),
            )
                .into_response();
        }
    };

    tracing::info!(%uid, token_name, "debug report requested");

    let report = match state.database_read.fetch_debug_report(&uid).await {
        Ok(Some(report)) => report,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                super::error("NotFound", "order not found"),
            )
                .into_response();
        }
        Err(err) => {
            tracing::error!(?err, "failed to fetch debug report");
            return crate::api::internal_error_reply();
        }
    };

    let sell_token = report.order.data.sell_token;
    let buy_token = report.order.data.buy_token;
    let auctions = build_auctions(&report, sell_token, buy_token);

    let response = DebugOrderResponse {
        order_uid: uid.to_string(),
        order: report.order,
        events: report.events.iter().map(DebugEvent::from).collect(),
        auctions,
        trades: report.trades.iter().map(DebugTrade::from).collect(),
    };
    (StatusCode::OK, Json(response)).into_response()
}

/// Groups auction-related data (prices, solutions, executions, settlement
/// attempts, fee policies) by auction ID into a single array sorted by ID.
fn build_auctions(
    report: &crate::database::debug_report::DebugReport,
    sell_token: Address,
    buy_token: Address,
) -> Vec<DebugAuction> {
    let sell = ByteArray(sell_token.0.0);
    let buy = ByteArray(buy_token.0.0);

    // Index all per-auction data by auction_id.
    let mut solutions_by_auction: HashMap<i64, Vec<DebugProposedSolution>> = HashMap::new();
    for s in &report.proposed_solutions {
        solutions_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(DebugProposedSolution::from(s));
    }

    let mut executions_by_auction: HashMap<i64, Vec<DebugExecution>> = HashMap::new();
    for e in &report.executions {
        executions_by_auction
            .entry(e.auction_id)
            .or_default()
            .push(DebugExecution::from(e));
    }

    let mut settlements_by_auction: HashMap<i64, Vec<DebugSettlementAttempt>> = HashMap::new();
    for s in &report.settlement_executions {
        settlements_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(DebugSettlementAttempt::from(s));
    }

    let mut fees_by_auction: HashMap<i64, Vec<DebugFeePolicy>> = HashMap::new();
    for f in &report.fee_policies {
        fees_by_auction
            .entry(f.auction_id)
            .or_default()
            .push(DebugFeePolicy::from(f));
    }

    let mut auctions: Vec<DebugAuction> = report
        .auctions
        .iter()
        .map(|a| {
            let native_prices: HashMap<String, String> = a
                .price_tokens
                .iter()
                .zip(&a.price_values)
                .filter(|(token, _)| **token == sell || **token == buy)
                .map(|(token, price)| (token.to_string(), price.to_string()))
                .collect();
            DebugAuction {
                id: a.id,
                block: a.block,
                deadline: a.deadline,
                native_prices,
                proposed_solutions: solutions_by_auction.remove(&a.id).unwrap_or_default(),
                executions: executions_by_auction.remove(&a.id).unwrap_or_default(),
                settlement_attempts: settlements_by_auction.remove(&a.id).unwrap_or_default(),
                fee_policies: fees_by_auction.remove(&a.id).unwrap_or_default(),
            }
        })
        .collect();

    auctions.sort_by_key(|a| a.id);
    auctions
}

/// Returns the token name if the x-auth-token header matches a configured
/// token. The map is keyed by secret -> name.
fn authenticate<'a>(headers: &HeaderMap, tokens: &'a HashMap<String, String>) -> Option<&'a str> {
    let header_value = headers.get("x-auth-token")?.to_str().ok()?;
    tokens.get(header_value).map(String::as_str)
}

fn format_bytes(bytes: &[u8]) -> String {
    const_hex::encode_prefixed(bytes)
}

// --- Response types ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOrderResponse {
    pub order_uid: String,
    pub order: Order,
    pub events: Vec<DebugEvent>,
    pub auctions: Vec<DebugAuction>,
    pub trades: Vec<DebugTrade>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugEvent {
    pub label: String,
    pub timestamp: String,
}

impl From<&OrderEvent> for DebugEvent {
    fn from(e: &OrderEvent) -> Self {
        Self {
            label: format!("{:?}", e.label).to_lowercase(),
            timestamp: e.timestamp.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugAuction {
    pub id: i64,
    pub block: i64,
    pub deadline: i64,
    pub native_prices: HashMap<String, String>,
    pub proposed_solutions: Vec<DebugProposedSolution>,
    pub executions: Vec<DebugExecution>,
    pub settlement_attempts: Vec<DebugSettlementAttempt>,
    pub fee_policies: Vec<DebugFeePolicy>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugProposedSolution {
    pub solution_uid: i64,
    pub ranking: i64,
    pub solver: String,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: String,
    pub executed_sell: String,
    pub executed_buy: String,
}

impl From<&OrderProposedSolution> for DebugProposedSolution {
    fn from(s: &OrderProposedSolution) -> Self {
        Self {
            solution_uid: s.solution_uid,
            ranking: s.ranking,
            solver: s.solver.to_string(),
            is_winner: s.is_winner,
            filtered_out: s.filtered_out,
            score: s.score.to_string(),
            executed_sell: s.executed_sell.to_string(),
            executed_buy: s.executed_buy.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugExecution {
    pub executed_fee: String,
    pub executed_fee_token: String,
    pub block_number: i64,
    pub protocol_fees: Vec<DebugProtocolFee>,
}

impl From<&OrderExecutionRow> for DebugExecution {
    fn from(e: &OrderExecutionRow) -> Self {
        let protocol_fees = e
            .protocol_fee_tokens
            .iter()
            .zip(e.protocol_fee_amounts.iter())
            .map(|(token, amount)| DebugProtocolFee {
                token: token.to_string(),
                amount: amount.to_string(),
            })
            .collect();
        Self {
            executed_fee: e.executed_fee.to_string(),
            executed_fee_token: e.executed_fee_token.to_string(),
            block_number: e.block_number,
            protocol_fees,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugProtocolFee {
    pub token: String,
    pub amount: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTrade {
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

impl From<&TradesQueryRow> for DebugTrade {
    fn from(t: &TradesQueryRow) -> Self {
        Self {
            block_number: t.block_number,
            log_index: t.log_index,
            buy_amount: t.buy_amount.to_string(),
            sell_amount: t.sell_amount.to_string(),
            sell_amount_before_fees: t.sell_amount_before_fees.to_string(),
            tx_hash: t.tx_hash.as_ref().map(|h| format_bytes(&h.0)),
            auction_id: t.auction_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSettlementAttempt {
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

impl From<&SettlementExecutionRow> for DebugSettlementAttempt {
    fn from(s: &SettlementExecutionRow) -> Self {
        Self {
            solver: s.solver.to_string(),
            solution_uid: s.solution_uid,
            start_timestamp: s.start_timestamp.to_rfc3339(),
            end_timestamp: s.end_timestamp.map(|t| t.to_rfc3339()),
            start_block: s.start_block,
            end_block: s.end_block,
            deadline_block: s.deadline_block,
            outcome: s.outcome.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugFeePolicy {
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

impl From<&FeePolicy> for DebugFeePolicy {
    fn from(f: &FeePolicy) -> Self {
        Self {
            kind: match f.kind {
                FeePolicyKind::Surplus => "surplus",
                FeePolicyKind::Volume => "volume",
                FeePolicyKind::PriceImprovement => "priceImprovement",
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
