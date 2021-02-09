use model::u256_decimal;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize)]
pub struct BatchAuctionModel {
    pub tokens: HashMap<String, TokenInfoModel>,
    pub orders: HashMap<String, OrderModel>,
    pub uniswaps: HashMap<String, UniswapModel>,
    pub ref_token: String,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub default_fee: f64,
}

#[derive(Debug, Serialize)]
pub struct OrderModel {
    pub sell_token: String,
    pub buy_token: String,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub allow_partial_fill: bool,
    pub is_sell_order: bool,
}

#[derive(Debug, Serialize)]
pub struct UniswapModel {
    pub token1: String,
    pub token2: String,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub balance1: u128,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub balance2: u128,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub fee: f64,
    pub mandatory: bool,
}

#[derive(Debug, Serialize)]
pub struct TokenInfoModel {
    pub decimals: u32,
}

#[derive(Debug, Deserialize)]
pub struct SettledBatchAuctionModel {
    pub orders: HashMap<String, ExecutedOrderModel>,
    pub uniswaps: HashMap<String, UpdatedUniswapModel>,
    pub ref_token: String,
    pub prices: HashMap<String, Price>,
}

#[derive(Debug, Deserialize)]
pub struct Price(#[serde(with = "serde_with::rust::display_fromstr")] pub f64);

#[derive(Debug, Deserialize)]
pub struct ExecutedOrderModel {
    #[serde(with = "u256_decimal")]
    pub exec_sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub exec_buy_amount: U256,
}

#[derive(Debug, Deserialize)]
pub struct UpdatedUniswapModel {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub balance_update_1: i128,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub balance_update_2: i128,
}
