use {
    crate::{
        domain::{competition, competition::order, eth, liquidity},
        util::{
            conv::{rational_to_big_decimal, u256::U256Ext},
            serialize,
        },
    },
    indexmap::IndexMap,
    number::U256,
    serde::Serialize,
    serde_with::serde_as,
    std::collections::{BTreeMap, HashMap},
};

impl Auction {
    pub fn new(
        auction: &competition::Auction,
        liquidity: &[liquidity::Liquidity],
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
                        reference_price: token.price.map(eth::U256::from).map(Into::into),
                        available_balance: token.available_balance.into(),
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
                .map(|order| {
                    let available = order.available(weth);
                    Order {
                        uid: order.uid.into(),
                        sell_token: available.sell.token.into(),
                        buy_token: available.buy.token.into(),
                        sell_amount: eth::U256::from(available.sell.amount).into(),
                        buy_amount: eth::U256::from(available.buy.amount).into(),
                        fee_amount: eth::U256::from(available.user_fee).into(),
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
                    }
                })
                .collect(),
            liquidity: liquidity
                .iter()
                .map(|liquidity| match &liquidity.kind {
                    liquidity::Kind::UniswapV2(pool) => {
                        Liquidity::ConstantProduct(ConstantProductPool {
                            id: liquidity.id.into(),
                            address: pool.address.into(),
                            gas_estimate: eth::U256::from(liquidity.gas).into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        ConstantProductReserve {
                                            balance: eth::U256::from(asset.amount).into(),
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
                            gas_estimate: liquidity.gas.0.into(),
                            tokens: vec![pool.tokens.get().0.into(), pool.tokens.get().1.into()],
                            sqrt_price: pool.sqrt_price.0.into(),
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
                        gas_estimate: eth::U256::from(liquidity.gas).into(),
                        tokens: pool
                            .reserves
                            .iter()
                            .map(|r| {
                                (
                                    r.asset.token.into(),
                                    StableReserve {
                                        balance: eth::U256::from(r.asset.amount).into(),
                                        scaling_factor: scaling_factor_to_decimal(r.scale),
                                    },
                                )
                            })
                            .collect(),
                        amplification_parameter: rational_to_big_decimal(&num::BigRational::new(
                            pool.amplification_parameter.factor().to_big_int(),
                            pool.amplification_parameter.precision().to_big_int(),
                        )),
                        fee: fee_to_decimal(pool.fee),
                    }),
                    liquidity::Kind::BalancerV2Weighted(pool) => {
                        Liquidity::WeightedProduct(WeightedProductPool {
                            id: liquidity.id.into(),
                            address: pool.id.address().into(),
                            gas_estimate: eth::U256::from(liquidity.gas).into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|r| {
                                    (
                                        r.asset.token.into(),
                                        WeightedProductReserve {
                                            balance: eth::U256::from(r.asset.amount).into(),
                                            scaling_factor: scaling_factor_to_decimal(r.scale),
                                            weight: weight_to_decimal(r.weight),
                                        },
                                    )
                                })
                                .collect(),
                            fee: fee_to_decimal(pool.fee),
                            version: match pool.version {
                                liquidity::balancer::v2::weighted::Version::V0 => {
                                    WeightedProductVersion::V0
                                }
                                liquidity::balancer::v2::weighted::Version::V3Plus => {
                                    WeightedProductVersion::V3Plus
                                }
                            },
                        })
                    }
                    liquidity::Kind::Swapr(pool) => {
                        Liquidity::ConstantProduct(ConstantProductPool {
                            id: liquidity.id.into(),
                            address: pool.base.address.into(),
                            gas_estimate: eth::U256::from(liquidity.gas).into(),
                            tokens: pool
                                .base
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        ConstantProductReserve {
                                            balance: eth::U256::from(asset.amount).into(),
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
            effective_gas_price: eth::U256::from(auction.gas_price().effective()).into(),
            deadline: auction.deadline().solvers(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    id: Option<String>,
    tokens: HashMap<eth::H160, Token>,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
    effective_gas_price: U256,
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
    sell_amount: U256,
    buy_amount: U256,
    fee_amount: U256,
    kind: Kind,
    partially_fillable: bool,
    class: Class,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Class {
    Market,
    Limit,
    Liquidity,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    decimals: Option<u8>,
    symbol: Option<String>,
    reference_price: Option<U256>,
    available_balance: U256,
    trusted: bool,
}

// TODO Remove dead_code
#[allow(dead_code, clippy::enum_variant_names)]
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
    gas_estimate: U256,
    tokens: BTreeMap<eth::H160, ConstantProductReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
}

#[derive(Debug, Serialize)]
struct ConstantProductReserve {
    balance: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductPool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    gas_estimate: U256,
    tokens: IndexMap<eth::H160, WeightedProductReserve>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    fee: bigdecimal::BigDecimal,
    version: WeightedProductVersion,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductReserve {
    balance: U256,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    scaling_factor: bigdecimal::BigDecimal,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    weight: bigdecimal::BigDecimal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum WeightedProductVersion {
    V0,
    V3Plus,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StablePool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    gas_estimate: U256,
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
    balance: U256,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    scaling_factor: bigdecimal::BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConcentratedLiquidityPool {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    address: eth::H160,
    gas_estimate: U256,
    tokens: Vec<eth::H160>,
    sqrt_price: U256,
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
    gas_estimate: U256,
    #[serde_as(as = "serialize::Hex")]
    hash: [u8; 32],
    maker_token: eth::H160,
    taker_token: eth::H160,
    maker_amount: U256,
    taker_amount: U256,
    taker_token_fee_amount: U256,
}

fn fee_to_decimal(fee: liquidity::balancer::v2::Fee) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(fee.as_raw().to_big_int(), 18)
}

fn weight_to_decimal(weight: liquidity::balancer::v2::weighted::Weight) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(weight.as_raw().to_big_int(), 18)
}

fn scaling_factor_to_decimal(
    scale: liquidity::balancer::v2::ScalingFactor,
) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(scale.as_raw().to_big_int(), 18)
}
