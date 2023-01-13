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
                .filter_map(|liquidity| match liquidity {
                    Liquidity::ConstantProduct(liquidity) => Some(liquidity.to_domain()),
                    Liquidity::WeightedProduct(liquidity) => Some(liquidity.to_domain()),
                    Liquidity::Stable(_)
                    | Liquidity::ConcentratedLiquidity(_)
                    | Liquidity::LimitOrder(_) => None,
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
                        scale: liquidity::weighted_product::ScalingFactor::new(
                            token.scaling_factor,
                        )
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
                fee: conv::decimal_to_rational(&self.fee).ok_or("invalid constant product fee")?,
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
