use crate::util::serialize;
use bigdecimal::BigDecimal;
use ethereum_types::{H160, U256};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;

impl Auction {}

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
