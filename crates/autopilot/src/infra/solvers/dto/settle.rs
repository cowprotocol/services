use {
    crate::infra::persistence::dto::order::Order,
    alloy::primitives::U256,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_with::{serde_as, skip_serializing_none},
};

#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Unique ID of the solution (per driver competition), to settle.
    pub solution_id: u64,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID in which the specified solution ID is competing.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
    /// Fast-path (out-of-competition) inputs. Present only when settling a
    /// cached quote solution against the real signed order.
    pub fast_path: Option<FastPath>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FastPath {
    /// The real signed order the cached solution is re-encoded against.
    pub order: Order,
    /// The sell/buy amounts the order must fill at exactly.
    pub limit_prices: LimitPrices,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitPrices {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy: U256,
}
