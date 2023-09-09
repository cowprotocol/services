use {
    crate::{
        api::dto::Error,
        domain::{auction, eth, liquidity, order},
        util::{conv, serialize},
    },
    bigdecimal::BigDecimal,
    ethereum_types::{H160, U256},
    itertools::Itertools as _,
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
    std::collections::HashMap,
};

impl Auction {
    /// Converts a data transfer object into its domain object representation.
    pub fn to_domain(&self) -> Result<auction::Auction, Error> {
        Ok(auction::Auction {
            id: self.id.map(auction::Id),
            tokens: auction::Tokens(
                self.tokens
                    .iter()
                    .map(|(address, token)| {
                        (
                            eth::TokenAddress(*address),
                            auction::Token {
                                decimals: token.decimals,
                                symbol: token.symbol.clone(),
                                reference_price: token
                                    .reference_price
                                    .map(eth::Ether)
                                    .map(auction::Price),
                                available_balance: token.available_balance,
                                trusted: token.trusted,
                            },
                        )
                    })
                    .collect(),
            ),
            orders: self
                .orders
                .iter()
                .map(|order| order::Order {
                    uid: order::Uid(order.uid),
                    sell: eth::Asset {
                        token: eth::TokenAddress(order.sell_token),
                        amount: order.sell_amount,
                    },
                    buy: eth::Asset {
                        token: eth::TokenAddress(order.buy_token),
                        amount: order.buy_amount,
                    },
                    side: match order.kind {
                        Kind::Buy => order::Side::Buy,
                        Kind::Sell => order::Side::Sell,
                    },
                    class: match order.class {
                        Class::Market => order::Class::Market,
                        Class::Limit => order::Class::Limit,
                        Class::Liquidity => order::Class::Liquidity,
                    },
                    fee: order::Fee(order.fee_amount),
                    partially_fillable: order.partially_fillable,
                })
                .collect(),
            liquidity: self
                .liquidity
                .iter()
                .map(|liquidity| match liquidity {
                    Liquidity::ConstantProduct(liquidity) => liquidity.to_domain(),
                    Liquidity::WeightedProduct(liquidity) => liquidity.to_domain(),
                    Liquidity::Stable(liquidity) => liquidity.to_domain(),
                    Liquidity::ConcentratedLiquidity(liquidity) => liquidity.to_domain(),
                    Liquidity::LimitOrder(liquidity) => Ok(liquidity.to_domain()),
                })
                .try_collect()?,
            gas_price: auction::GasPrice(eth::Ether(self.effective_gas_price)),
            deadline: auction::Deadline(self.deadline),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Auction {
    #[serde_as(as = "Option<DisplayFromStr>")]
    id: Option<i64>,
    tokens: HashMap<H160, Token>,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: U256,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Class {
    Market,
    Limit,
    Liquidity,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Token {
    decimals: Option<u8>,
    symbol: Option<String>,
    #[serde_as(as = "Option<serialize::U256>")]
    reference_price: Option<U256>,
    #[serde_as(as = "serialize::U256")]
    available_balance: U256,
    trusted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstantProductPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, ConstantProductReserve>,
    fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstantProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
}

impl ConstantProductPool {
    fn to_domain(&self) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let (a, b) = self
                .tokens
                .iter()
                .map(|(token, reserve)| eth::Asset {
                    token: eth::TokenAddress(*token),
                    amount: reserve.balance,
                })
                .collect_tuple()
                .ok_or("invalid number of constant product tokens")?;
            liquidity::constant_product::Reserves::new(a, b)
                .ok_or("invalid constant product pool reserves")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate),
            state: liquidity::State::ConstantProduct(liquidity::constant_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid constant product fee")?,
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WeightedProductPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, WeightedProductReserve>,
    fee: BigDecimal,
    version: WeightedProductVersion,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WeightedProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    scaling_factor: BigDecimal,
    weight: BigDecimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WeightedProductVersion {
    V0,
    V3Plus,
}

impl WeightedProductPool {
    fn to_domain(&self) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let entries = self
                .tokens
                .iter()
                .map(|(address, token)| {
                    Ok(liquidity::weighted_product::Reserve {
                        asset: eth::Asset {
                            token: eth::TokenAddress(*address),
                            amount: token.balance,
                        },
                        weight: conv::decimal_to_rational(&token.weight)
                            .ok_or("invalid token weight")?,
                        scale: conv::decimal_to_rational(&token.scaling_factor)
                            .and_then(liquidity::ScalingFactor::new)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::weighted_product::Reserves::new(entries)
                .ok_or("duplicate weighted token addresses")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate),
            state: liquidity::State::WeightedProduct(liquidity::weighted_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid weighted product fee")?,
                version: match self.version {
                    WeightedProductVersion::V0 => liquidity::weighted_product::Version::V0,
                    WeightedProductVersion::V3Plus => liquidity::weighted_product::Version::V3Plus,
                },
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StablePool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: HashMap<H160, StableReserve>,
    amplification_parameter: BigDecimal,
    fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StableReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    scaling_factor: BigDecimal,
}

impl StablePool {
    fn to_domain(&self) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let entries = self
                .tokens
                .iter()
                .map(|(address, token)| {
                    Ok(liquidity::stable::Reserve {
                        asset: eth::Asset {
                            token: eth::TokenAddress(*address),
                            amount: token.balance,
                        },
                        scale: conv::decimal_to_rational(&token.scaling_factor)
                            .and_then(liquidity::ScalingFactor::new)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::stable::Reserves::new(entries).ok_or("duplicate stable token addresses")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate),
            state: liquidity::State::Stable(liquidity::stable::Pool {
                reserves,
                amplification_parameter: conv::decimal_to_rational(&self.amplification_parameter)
                    .ok_or("invalid amplification parameter")?,
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid stable pool fee")?,
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConcentratedLiquidityPool {
    id: String,
    address: H160,
    #[serde_as(as = "serialize::U256")]
    gas_estimate: U256,
    tokens: Vec<H160>,
    #[serde_as(as = "serialize::U256")]
    sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    liquidity: u128,
    tick: i32,
    #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")]
    liquidity_net: HashMap<i32, i128>,
    fee: BigDecimal,
}

impl ConcentratedLiquidityPool {
    fn to_domain(&self) -> Result<liquidity::Liquidity, Error> {
        let tokens = {
            let (a, b) = self
                .tokens
                .iter()
                .copied()
                .map(eth::TokenAddress)
                .collect_tuple()
                .ok_or("invalid number of concentrated liquidity pool tokens")?;
            liquidity::TokenPair::new(a, b)
                .ok_or("duplicate concentrated liquidity pool token address")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate),
            state: liquidity::State::Concentrated(liquidity::concentrated::Pool {
                tokens,
                sqrt_price: liquidity::concentrated::SqrtPrice(self.sqrt_price),
                liquidity: liquidity::concentrated::Amount(self.liquidity),
                tick: liquidity::concentrated::Tick(self.tick),
                liquidity_net: self
                    .liquidity_net
                    .iter()
                    .map(|(tick, liquidity)| {
                        (
                            liquidity::concentrated::Tick(*tick),
                            liquidity::concentrated::LiquidityNet(*liquidity),
                        )
                    })
                    .collect(),
                fee: liquidity::concentrated::Fee(
                    conv::decimal_to_rational(&self.fee)
                        .ok_or("invalid concentrated liquidity pool fee")?,
                ),
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl ForeignLimitOrder {
    fn to_domain(&self) -> liquidity::Liquidity {
        liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate),
            state: liquidity::State::LimitOrder(liquidity::limit_order::LimitOrder {
                maker: eth::Asset {
                    token: eth::TokenAddress(self.maker_token),
                    amount: self.maker_amount,
                },
                taker: eth::Asset {
                    token: eth::TokenAddress(self.taker_token),
                    amount: self.taker_amount,
                },
                fee: liquidity::limit_order::TakerAmount(self.taker_token_fee_amount),
            }),
        }
    }
}
