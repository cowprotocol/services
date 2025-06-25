use {
    crate::{auction::AuctionId, order::OrderUid},
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

/// Stored directly in the database and turned into SolverCompetitionAPI for the
/// `/solver_competition` endpoint.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub auction_id: AuctionId,
    pub auction_start_block: i64,
    pub transaction_hash: Vec<H256>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub reference_scores: BTreeMap<H160, U256>,
    pub auction: Auction,
    pub solutions: Vec<Solution>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub orders: Vec<OrderUid>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
}

#[serde_as]
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub solver_address: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub score: U256,
    pub ranking: usize,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub clearing_prices: BTreeMap<H160, U256>,
    pub orders: Vec<Order>,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub tx_hash: Option<H256>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub reference_score: Option<U256>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde_as]
pub struct Order {
    pub id: OrderUid,
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
}
