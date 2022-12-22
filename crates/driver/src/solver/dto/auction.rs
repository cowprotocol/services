use {
    crate::{logic::competition, util::serialize},
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Auction {
    pub fn new(
        auction: &competition::Auction,
        deadline: competition::auction::SolverDeadline,
    ) -> Self {
        Self {
            id: auction.id.map(|id| id.0.to_string()),
            tokens: auction
                .tokens
                .iter()
                .map(|token| {
                    (
                        token.address.into(),
                        Token {
                            decimals: token.decimals,
                            symbol: token.symbol.clone(),
                            reference_price: token.price.map(Into::into),
                            available_balance: token.available_balance,
                            trusted: token.trusted,
                        },
                    )
                })
                .collect(),
            orders: auction
                .orders
                .iter()
                .map(|order| Order {
                    uid: order.uid.into(),
                    sell_token: order.sell.token.into(),
                    buy_token: order.buy.token.into(),
                    sell_amount: order.sell.amount,
                    buy_amount: order.buy.amount,
                    fee_amount: order.fee.solver.into(),
                    kind: match order.side {
                        competition::order::Side::Buy => Kind::Buy,
                        competition::order::Side::Sell => Kind::Sell,
                    },
                    partially_fillable: order.is_partial(),
                    class: match order.kind {
                        competition::order::Kind::Market => Class::Market,
                        competition::order::Kind::Limit { .. } => Class::Limit,
                        competition::order::Kind::Liquidity => Class::Liquidity,
                    },
                    reward: order.reward,
                })
                .collect(),
            // TODO Implement this when you do liquidity
            liquidity: vec![],
            effective_gas_price: auction.gas_price.into(),
            deadline: deadline.into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    id: Option<String>,
    tokens: HashMap<H160, Token>,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: U256,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    #[serde_as(as = "serialize::Hex")]
    uid: [u8; 56],
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: U256,
    #[serde_as(as = "serialize::U256")]
    fee_amount: U256,
    kind: Kind,
    partially_fillable: bool,
    class: Class,
    reward: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Class {
    Market,
    Limit,
    Liquidity,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    decimals: Option<u8>,
    symbol: Option<String>,
    #[serde_as(as = "Option<serialize::U256>")]
    reference_price: Option<U256>,
    #[serde_as(as = "serialize::U256")]
    available_balance: U256,
    trusted: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConstantProductPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, ConstantProductReserve>,
    fee: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct ConstantProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, WeightedProductReserve>,
    fee: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct WeightedProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    weight: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StablePool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, StableReserve>,
    amplification_parameter: f64,
    fee: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StableReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    #[serde_as(as = "serialize::U256")]
    scaling_factor: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConcentratedLiquidityPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: Vec<H160>,
    #[serde_as(as = "serialize::U256")]
    sqrt_price: U256,
    #[serde_as(as = "serialize::U256")]
    liquidity: U256,
    #[serde_as(as = "serialize::U256")]
    tick: U256,
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    liquidity_net: HashMap<usize, U256>,
    fee: f64,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForeignLimitOrder {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    #[serde_as(as = "serialize::Hex")]
    hash: [u8; 32],
    maker_token: H160,
    taker_token: H160,
    #[serde_as(as = "serialize::U256")]
    maker_amount: U256,
    #[serde_as(as = "serialize::U256")]
    taker_amount: U256,
    #[serde_as(as = "serialize::U256")]
    taker_token_fee_amount: U256,
}
