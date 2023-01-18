use {
    crate::{
        domain::{competition, eth, liquidity},
        infra,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Auction {
    pub fn from_domain(
        auction: &competition::Auction,
        timeout: competition::SolverTimeout,
        now: infra::time::Now,
    ) -> Self {
        Self {
            id: auction.id.as_ref().map(ToString::to_string),
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
            liquidity: auction
                .liquidity
                .iter()
                .map(|liquidity| match &liquidity.data {
                    liquidity::Data::UniswapV2(pool) => {
                        Liquidity::ConstantProduct(ConstantProductPool {
                            id: liquidity.id.into(),
                            address: liquidity.address.into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        ConstantProductReserve {
                                            balance: asset.amount,
                                        },
                                    )
                                })
                                .collect(),
                            fee: bigdecimal::BigDecimal::new(3.into(), 3),
                        })
                    }
                    liquidity::Data::UniswapV3(_) => todo!(),
                    liquidity::Data::BalancerV2Stable(_) => todo!(),
                    liquidity::Data::BalancerV2Weighted(_) => todo!(),
                    liquidity::Data::Swapr(_) => todo!(),
                    liquidity::Data::ZeroEx(_) => todo!(),
                })
                .collect(),
            effective_gas_price: auction.gas_price.into(),
            deadline: timeout.deadline(now),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    id: Option<String>,
    tokens: HashMap<eth::H160, Token>,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: eth::U256,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    #[serde_as(as = "serialize::Hex")]
    uid: [u8; 56],
    sell_token: eth::H160,
    buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    fee_amount: eth::U256,
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
    reference_price: Option<eth::U256>,
    #[serde_as(as = "serialize::U256")]
    available_balance: eth::U256,
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
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: eth::U256,
    tokens: HashMap<eth::H160, ConstantProductReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct ConstantProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: eth::U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductPool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: eth::U256,
    tokens: HashMap<eth::H160, WeightedProductReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct WeightedProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: eth::U256,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    weight: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StablePool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: eth::U256,
    tokens: HashMap<eth::H160, StableReserve>,
    amplification_parameter: f64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StableReserve {
    #[serde_as(as = "serialize::U256")]
    balance: eth::U256,
    #[serde_as(as = "serialize::U256")]
    scaling_factor: eth::U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConcentratedLiquidityPool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: eth::U256,
    tokens: Vec<eth::H160>,
    #[serde_as(as = "serialize::U256")]
    sqrt_price: eth::U256,
    #[serde_as(as = "serialize::U256")]
    liquidity: eth::U256,
    tick: i32,
    #[serde_as(as = "HashMap<serde_with::DisplayFromStr, serialize::U256>")]
    liquidity_net: HashMap<i32, eth::U256>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ForeignLimitOrder {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: eth::U256,
    #[serde_as(as = "serialize::Hex")]
    hash: [u8; 32],
    maker_token: eth::H160,
    taker_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    maker_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    taker_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    taker_token_fee_amount: eth::U256,
}
