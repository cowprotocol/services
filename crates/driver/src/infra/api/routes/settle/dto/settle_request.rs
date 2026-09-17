use {
    crate::infra::api::routes::solve::dto::solve_request::Order,
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
};

/// Request to the `/settle` endpoint: either a solution proposed in an
/// auction, or a fast-path quote solution cached by `/quote`.
///
/// Untagged: an auction settlement is recognised by its `solutionId`, a
/// fast-path settlement by its `quoteId`, `order` and `limitPrices`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SettleRequest {
    Auction(AuctionSettleRequest),
    FastPath(Box<FastPathSettleRequest>),
}

impl SettleRequest {
    pub fn auction_id(&self) -> i64 {
        match self {
            Self::Auction(request) => request.auction_id,
            Self::FastPath(request) => request.auction_id,
        }
    }

    pub fn submission_deadline_latest_block(&self) -> u64 {
        match self {
            Self::Auction(request) => request.submission_deadline_latest_block,
            Self::FastPath(request) => request.submission_deadline_latest_block,
        }
    }
}

/// Settle a solution the driver proposed in `/solve` for the auction.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionSettleRequest {
    /// Unique ID of the solution (per driver competition), to settle.
    pub solution_id: u64,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID in which this solution is competing.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
}

/// Settle the quote solution cached under `quote_id`, re-encoded against the
/// real signed order, outside of a competition.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastPathSettleRequest {
    /// Id of the quote whose cached solution should be settled.
    pub quote_id: i64,
    /// The real signed order the cached solution is re-encoded against.
    pub order: Order,
    /// The sell/buy amounts defining the exact price the order must fill at.
    pub limit_prices: LimitPrices,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID the autopilot allocated for this settlement. The quote was
    /// solved outside of any auction, but its settlement is still attributed
    /// to one.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitPrices {
    #[serde_as(as = "serde_ext::U256")]
    pub sell: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    pub buy: eth::U256,
}
