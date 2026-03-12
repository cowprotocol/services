use {
    crate::order::{Order, OrderUid},
    alloy_primitives::{Address, B256},
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct DebugReport {
    pub order_uid: OrderUid,
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
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: i64,
    pub block: i64,
    pub deadline: i64,
    #[serde_as(as = "HashMap<_, DisplayFromStr>")]
    pub native_prices: HashMap<Address, BigDecimal>,
    pub proposed_solutions: Vec<ProposedSolution>,
    pub executions: Vec<Execution>,
    pub settlement_attempts: Vec<SettlementAttempt>,
    pub fee_policies: Vec<FeePolicy>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct ProposedSolution {
    pub solution_uid: i64,
    pub ranking: i64,
    pub solver: Address,
    pub is_winner: bool,
    pub filtered_out: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub score: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub executed_sell: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub executed_buy: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde_as(as = "DisplayFromStr")]
    pub executed_fee: BigDecimal,
    pub executed_fee_token: Address,
    pub block_number: i64,
    pub protocol_fees: Vec<ProtocolFee>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct ProtocolFee {
    pub token: Address,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub block_number: i64,
    pub log_index: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount_before_fees: BigDecimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<B256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auction_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct SettlementAttempt {
    pub solver: Address,
    pub solution_uid: i64,
    pub start_timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_timestamp: Option<DateTime<Utc>>,
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
    pub kind: FeePolicyKind,
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

#[derive(Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub enum FeePolicyKind {
    Surplus,
    Volume,
    PriceImprovement,
}
