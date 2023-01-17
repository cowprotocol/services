use crate::{
    api::dto::Error,
    domain::{auction, eth, liquidity, order},
    util::{conv, serialize},
};
use bigdecimal::BigDecimal;
use ethereum_types::{H160, U256};
use itertools::Itertools as _;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;

impl Auction {
    /// Converts a data transfer object into its domain object representation.
    pub fn to_domain(&self) -> Result<auction::Auction, Error> {
        Ok(auction::Auction {
            orders: self
                .orders
                .iter()
                .map(|order| -> order::Order {
                    order::Order {
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
                    }
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
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
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

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstantProductPool {
    id: String,
    address: H160,
    gas_estimate: u64,
    tokens: HashMap<H160, ConstantProductReserve>,
    fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
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
                .ok_or("duplicate constant product token address")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate.into()),
            state: liquidity::State::ConstantProduct(liquidity::constant_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid constant product fee")?,
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeightedProductPool {
    id: String,
    address: H160,
    gas_estimate: u64,
    tokens: HashMap<H160, WeightedProductReserve>,
    fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct WeightedProductReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    #[serde_as(as = "serialize::U256")]
    scaling_factor: U256,
    weight: BigDecimal,
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
                        scale: liquidity::ScalingFactor::new(token.scaling_factor)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::weighted_product::Reserves::new(entries)
                .ok_or("duplicate weighted token addresss")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate.into()),
            state: liquidity::State::WeightedProduct(liquidity::weighted_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid weighted product fee")?,
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StablePool {
    id: String,
    address: H160,
    gas_estimate: u64,
    tokens: HashMap<H160, StableReserve>,
    amplification_parameter: BigDecimal,
    fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StableReserve {
    #[serde_as(as = "serialize::U256")]
    balance: U256,
    #[serde_as(as = "serialize::U256")]
    scaling_factor: U256,
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
                        scale: liquidity::ScalingFactor::new(token.scaling_factor)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::stable::Reserves::new(entries).ok_or("duplicate stable token addresss")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(self.id.clone()),
            address: self.address,
            gas: eth::Gas(self.gas_estimate.into()),
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
#[serde(rename_all = "camelCase")]
struct ConcentratedLiquidityPool {
    id: String,
    address: H160,
    gas_estimate: u64,
    tokens: Vec<H160>,
    #[serde_as(as = "serialize::U256")]
    sqrt_price: U256,
    #[serde_as(as = "serialize::U256")]
    liquidity: U256,
    tick: i32,
    #[serde_as(as = "HashMap<DisplayFromStr, serialize::U256>")]
    liquidity_net: HashMap<i32, U256>,
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
            gas: eth::Gas(self.gas_estimate.into()),
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
                            liquidity::concentrated::Amount(*liquidity),
                        )
                    })
                    .collect(),
                fee: conv::decimal_to_rational(&self.fee)
                    .ok_or("invalid concentrated liquidity pool fee")?,
            }),
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForeignLimitOrder {
    id: String,
    address: H160,
    gas_estimate: u64,
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
            gas: eth::Gas(self.gas_estimate.into()),
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
