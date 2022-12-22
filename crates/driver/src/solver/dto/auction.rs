use {
    crate::{logic, util::serialize},
    ethereum_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::{BTreeMap, HashMap},
};

// TODO Since building the auction will also require liquidity later down the
// line, this is probably not good enough. But that will be implemented when the
// `logic::liquidity` module is added.
impl Auction {
    pub fn new(_auction: &logic::competition::Auction) -> Self {
        todo!()
    }
}

#[derive(Debug, Serialize)]
pub struct Auction {
    tokens: HashMap<H160, Token>,
    orders: HashMap<usize, Order>,
    amms: BTreeMap<H160, Amm>,
    metadata: Option<Metadata>,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct Order {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: U256,
    allow_partial_fill: bool,
    is_sell_order: bool,
    fee: TokenAmount,
    cost: TokenAmount,
    is_liquidity_order: bool,
    is_mature: bool,
    mandatory: bool,
    has_atomic_execution: bool,
    reward: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct TokenAmount {
    #[serde_as(as = "serialize::U256")]
    amount: U256,
    token: H160,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct Token {
    decimals: Option<u8>,
    alias: Option<String>,
    external_price: Option<f64>,
    normalize_priority: Option<u64>,
    #[serde_as(as = "Option<serialize::U256>")]
    internal_buffer: Option<U256>,
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
