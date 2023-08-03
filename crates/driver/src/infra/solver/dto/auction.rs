use {
    crate::{
        domain::{competition, competition::order, eth, liquidity},
        util::serialize,
    },
    indexmap::IndexMap,
    number_conversions::{rational_to_big_decimal, u256_to_big_int},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::{BTreeMap, HashMap},
};

impl Auction {
    pub fn new(
        auction: &competition::Auction,
        liquidity: &[liquidity::Liquidity],
        timeout: competition::SolverTimeout,
        weth: eth::WethAddress,
    ) -> Self {
        let mut tokens: HashMap<eth::H160, _> = auction
            .tokens()
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
            .collect();

        // Make sure that we have at least empty entries for all tokens for
        // which we are providing liquidity.
        for token in liquidity
            .iter()
            .flat_map(|liquidity| match &liquidity.kind {
                liquidity::Kind::UniswapV2(pool) => pool.reserves.iter().map(|r| r.token).collect(),
                liquidity::Kind::UniswapV3(pool) => vec![pool.tokens.get().0, pool.tokens.get().1],
                liquidity::Kind::BalancerV2Stable(pool) => pool.reserves.tokens().collect(),
                liquidity::Kind::BalancerV2Weighted(pool) => pool.reserves.tokens().collect(),
                liquidity::Kind::Swapr(pool) => {
                    pool.base.reserves.iter().map(|r| r.token).collect()
                }
                liquidity::Kind::ZeroEx(_) => todo!(),
            })
        {
            tokens.entry(token.into()).or_insert_with(Default::default);
        }

        Self {
            id: auction.id().as_ref().map(ToString::to_string),
            orders: auction
                .orders()
                .iter()
                .map(|order| Order {
                    uid: order.uid.into(),
                    sell_token: order.sell.token.into(),
                    buy_token: order.solver_buy(weth).token.into(),
                    sell_amount: order.sell.amount.into(),
                    buy_amount: order.solver_buy(weth).amount.into(),
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
                })
                .collect(),
            liquidity: liquidity
                .iter()
                .map(|liquidity| match &liquidity.kind {
                    liquidity::Kind::UniswapV2(pool) => {
                        Liquidity::ConstantProduct(ConstantProductPool {
                            id: liquidity.id.into(),
                            address: pool.address.into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        ConstantProductReserve {
                                            balance: asset.amount.into(),
                                        },
                                    )
                                })
                                .collect(),
                            fee: bigdecimal::BigDecimal::new(3.into(), 3),
                        })
                    }
                    liquidity::Kind::UniswapV3(pool) => {
                        Liquidity::ConcentratedLiquidity(ConcentratedLiquidityPool {
                            id: liquidity.id.into(),
                            address: pool.address.0,
                            gas_estimate: liquidity.gas.0,
                            tokens: vec![pool.tokens.get().0.into(), pool.tokens.get().1.into()],
                            sqrt_price: pool.sqrt_price.0,
                            liquidity: pool.liquidity.0,
                            tick: pool.tick.0,
                            liquidity_net: pool
                                .liquidity_net
                                .iter()
                                .map(|(key, value)| (key.0, value.0))
                                .collect(),
                            fee: rational_to_big_decimal(&pool.fee.0),
                        })
                    }
                    liquidity::Kind::BalancerV2Stable(pool) => Liquidity::Stable(StablePool {
                        id: liquidity.id.into(),
                        address: pool.id.address().into(),
                        gas_estimate: liquidity.gas.into(),
                        tokens: pool
                            .reserves
                            .iter()
                            .map(|r| {
                                (
                                    r.asset.token.into(),
                                    StableReserve {
                                        balance: r.asset.amount.into(),
                                        scaling_factor: r.scale.factor(),
                                    },
                                )
                            })
                            .collect(),
                        amplification_parameter: rational_to_big_decimal(&num::BigRational::new(
                            u256_to_big_int(&pool.amplification_parameter.factor()),
                            u256_to_big_int(&pool.amplification_parameter.precision()),
                        )),
                        fee: bigdecimal::BigDecimal::new(u256_to_big_int(&pool.fee.into()), 18),
                    }),
                    liquidity::Kind::BalancerV2Weighted(pool) => {
                        Liquidity::WeightedProduct(WeightedProductPool {
                            id: liquidity.id.into(),
                            address: pool.id.address().into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|r| {
                                    (
                                        r.asset.token.into(),
                                        WeightedProductReserve {
                                            balance: r.asset.amount.into(),
                                            scaling_factor: r.scale.factor(),
                                            weight: bigdecimal::BigDecimal::new(
                                                u256_to_big_int(&r.weight.into()),
                                                18,
                                            ),
                                        },
                                    )
                                })
                                .collect(),
                            fee: bigdecimal::BigDecimal::new(u256_to_big_int(&pool.fee.into()), 18),
                        })
                    }
                    liquidity::Kind::Swapr(pool) => {
                        Liquidity::ConstantProduct(ConstantProductPool {
                            id: liquidity.id.into(),
                            address: pool.base.address.into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .base
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        ConstantProductReserve {
                                            balance: asset.amount.into(),
                                        },
                                    )
                                })
                                .collect(),
                            fee: bigdecimal::BigDecimal::new(pool.fee.bps().into(), 4),
                        })
                    }
                    liquidity::Kind::ZeroEx(_) => todo!(),
                })
                .collect(),
            tokens,
            effective_gas_price: auction.gas_price().effective().into(),
            deadline: timeout.deadline(),
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
    uid: [u8; order::UID_LEN],
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
#[derive(Default, Debug, Serialize)]
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

// TODO Remove dead_code
#[allow(dead_code)]
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
    tokens: BTreeMap<eth::H160, ConstantProductReserve>,
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
    tokens: IndexMap<eth::H160, WeightedProductReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: eth::U256,
    #[serde_as(as = "serialize::U256")]
    scaling_factor: eth::U256,
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
    tokens: IndexMap<eth::H160, StableReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    amplification_parameter: bigdecimal::BigDecimal,
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
    #[serde_as(as = "serde_with::DisplayFromStr")]
    liquidity: u128,
    tick: i32,
    #[serde_as(as = "BTreeMap<serde_with::DisplayFromStr, serde_with::DisplayFromStr>")]
    liquidity_net: BTreeMap<i32, i128>,
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
