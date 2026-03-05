use {
    crate::api::AppState,
    axum::{
        extract::{Path, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Json, Response},
    },
    database::{
        auction::Auction as DbAuction,
        byte_array::ByteArray,
        order_events::OrderEvent,
        order_execution::ExecutionRow as OrderExecutionRow,
        orders::{Order as DbOrder, Quote as DbQuote},
        settlement_executions::ExecutionRow as SettlementExecutionRow,
        solver_competition_v2::OrderProposedSolution,
        trades::TradesQueryRow,
    },
    model::order::OrderUid,
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

    let sell_token = report.order.sell_token;
    let buy_token = report.order.buy_token;

    let response = DebugOrderResponse {
        order_uid: uid.to_string(),
        order: DebugOrderDetails::from(&report.order),
        quote: report.quote.as_ref().map(DebugQuote::from),
        events: report.events.iter().map(DebugEvent::from).collect(),
        auctions: report
            .auctions
            .iter()
            .map(|a| DebugAuction::from_auction(a, sell_token, buy_token))
            .collect(),
        proposed_solutions: report
            .proposed_solutions
            .iter()
            .map(DebugProposedSolution::from)
            .collect(),
        executions: report.executions.iter().map(DebugExecution::from).collect(),
        trades: report.trades.iter().map(DebugTrade::from).collect(),
        settlement_attempts: report
            .settlement_executions
            .iter()
            .map(DebugSettlementAttempt::from)
            .collect(),
    };
    (StatusCode::OK, Json(response)).into_response()
}

/// Returns the token name if the x-auth-token header matches a configured
/// token. The map is keyed by secret -> name.
fn authenticate<'a>(headers: &HeaderMap, tokens: &'a HashMap<String, String>) -> Option<&'a str> {
    let header_value = headers.get("x-auth-token")?.to_str().ok()?;
    tokens.get(header_value).map(String::as_str)
}

fn format_address(addr: &ByteArray<20>) -> String {
    format!("0x{}", const_hex::encode(addr.0))
}

fn format_bytes(bytes: &[u8]) -> String {
    format!("0x{}", const_hex::encode(bytes))
}

// --- Response types ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOrderResponse {
    pub order_uid: String,
    pub order: DebugOrderDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<DebugQuote>,
    pub events: Vec<DebugEvent>,
    pub auctions: Vec<DebugAuction>,
    pub proposed_solutions: Vec<DebugProposedSolution>,
    pub executions: Vec<DebugExecution>,
    pub trades: Vec<DebugTrade>,
    pub settlement_attempts: Vec<DebugSettlementAttempt>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOrderDetails {
    pub owner: String,
    pub sell_token: String,
    pub buy_token: String,
    pub sell_amount: String,
    pub buy_amount: String,
    pub valid_to: i64,
    pub kind: String,
    pub class: String,
    pub partially_fillable: bool,
    pub creation_timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_timestamp: Option<String>,
    pub signing_scheme: String,
    pub app_data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
}

impl From<&DbOrder> for DebugOrderDetails {
    fn from(o: &DbOrder) -> Self {
        Self {
            owner: format_address(&o.owner),
            sell_token: format_address(&o.sell_token),
            buy_token: format_address(&o.buy_token),
            sell_amount: o.sell_amount.to_string(),
            buy_amount: o.buy_amount.to_string(),
            valid_to: o.valid_to,
            kind: format!("{:?}", o.kind).to_lowercase(),
            class: format!("{:?}", o.class).to_lowercase(),
            partially_fillable: o.partially_fillable,
            creation_timestamp: o.creation_timestamp.to_rfc3339(),
            cancellation_timestamp: o.cancellation_timestamp.map(|t| t.to_rfc3339()),
            signing_scheme: format!("{:?}", o.signing_scheme),
            app_data: format_bytes(&o.app_data.0),
            receiver: o.receiver.as_ref().map(format_address),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugQuote {
    pub gas_amount: f64,
    pub gas_price: f64,
    pub sell_token_price: f64,
    pub sell_amount: String,
    pub buy_amount: String,
    pub solver: String,
    pub verified: bool,
}

impl From<&DbQuote> for DebugQuote {
    fn from(q: &DbQuote) -> Self {
        Self {
            gas_amount: q.gas_amount,
            gas_price: q.gas_price,
            sell_token_price: q.sell_token_price,
            sell_amount: q.sell_amount.to_string(),
            buy_amount: q.buy_amount.to_string(),
            solver: format_address(&q.solver),
            verified: q.verified,
        }
    }
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
}

impl DebugAuction {
    fn from_auction(a: &DbAuction, sell_token: ByteArray<20>, buy_token: ByteArray<20>) -> Self {
        let mut native_prices = HashMap::new();
        for (token, price) in a.price_tokens.iter().zip(a.price_values.iter()) {
            if *token == sell_token || *token == buy_token {
                native_prices.insert(format_address(token), price.to_string());
            }
        }
        Self {
            id: a.id,
            block: a.block,
            deadline: a.deadline,
            native_prices,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugProposedSolution {
    pub auction_id: i64,
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
            auction_id: s.auction_id,
            solution_uid: s.solution_uid,
            ranking: s.ranking,
            solver: format_address(&s.solver),
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
    pub auction_id: i64,
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
                token: format_address(token),
                amount: amount.to_string(),
            })
            .collect();
        Self {
            auction_id: e.auction_id,
            executed_fee: e.executed_fee.to_string(),
            executed_fee_token: format_address(&e.executed_fee_token),
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
    pub auction_id: i64,
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
            auction_id: s.auction_id,
            solver: format_address(&s.solver),
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
