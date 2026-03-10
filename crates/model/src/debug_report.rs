use {crate::order::Order, serde::Serialize, std::collections::HashMap};

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct DebugReport {
    pub order_uid: String,
    pub order: Order,
    pub events: Vec<Event>,
    pub auctions: Vec<Auction>,
    pub trades: Vec<Trade>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub label: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub executed_fee: String,
    pub executed_fee_token: String,
    pub block_number: i64,
    pub protocol_fees: Vec<ProtocolFee>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct ProtocolFee {
    pub token: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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
