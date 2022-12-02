use {
    crate::{logic::solution, util::serialize},
    ethereum_types::H160,
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeMap,
};

impl From<solution::Auction> for Auction {
    fn from(auction: solution::Auction) -> Self {
        todo!()
    }
}

#[derive(Debug, Serialize)]
pub struct Auction {
    tokens: BTreeMap<H160, Token>,
    orders: BTreeMap<usize, Order>,
    amms: BTreeMap<H160, Amm>,
    metadata: Option<Metadata>,
}

#[derive(Debug, Serialize)]
struct Order {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    sell_token: H160,
    buy_token: H160,
    sell_amount: serialize::U256,
    buy_amount: serialize::U256,
    allow_partial_fill: bool,
    is_sell_order: bool,
    fee: TokenAmount,
    cost: TokenAmount,
    is_liquidity_order: bool,
    is_mature: bool,
    #[serde(default)]
    mandatory: bool,
    has_atomic_execution: bool,
    reward: f64,
}

#[derive(Debug, Serialize)]
struct TokenAmount {
    amount: serialize::U256,
    token: H160,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct Token {
    decimals: Option<u8>,
    alias: Option<String>,
    external_price: Option<f64>,
    normalize_priority: Option<u64>,
    internal_buffer: Option<serialize::U256>,
    /// Is token in the external list containing only safe tokens
    accepted_for_internalization: bool,
}

#[derive(Debug, Serialize)]
struct Amm {}

#[derive(Debug, Serialize)]
struct Metadata {
    environment: Option<String>,
    auction_id: Option<i64>,
    run_id: Option<u64>,
    gas_price: Option<f64>,
    native_token: Option<H160>,
}
