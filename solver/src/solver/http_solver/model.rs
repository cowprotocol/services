use model::u256_decimal;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize)]
pub struct BatchAuctionModel {
    pub tokens: HashMap<String, TokenInfoModel>,
    pub orders: HashMap<String, OrderModel>,
    pub uniswaps: HashMap<String, UniswapModel>,
    pub metadata: Option<MetadataModel>,
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
    pub fee: FeeModel,
    pub cost: CostModel,
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
    pub cost: CostModel,
    pub mandatory: bool,
}

#[derive(Debug, Serialize)]
pub struct TokenInfoModel {
    pub decimals: Option<u32>,
    pub external_price: Option<f64>,
    pub normalize_priority: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CostModel {
    pub amount: u128,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct FeeModel {
    pub amount: u128,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct SettledBatchAuctionModel {
    pub orders: HashMap<String, ExecutedOrderModel>,
    pub uniswaps: HashMap<String, UpdatedUniswapModel>,
    pub ref_token: String,
    pub prices: HashMap<String, Price>,
}

impl SettledBatchAuctionModel {
    pub fn has_execution_plan(&self) -> bool {
        self.uniswaps
            .values()
            .into_iter()
            .all(|u| u.exec_plan.is_some())
    }
}

#[derive(Debug, Deserialize)]
pub struct Price(#[serde(with = "serde_with::rust::display_fromstr")] pub f64);

#[derive(Debug, Serialize)]
pub struct MetadataModel {
    pub environment: Option<String>,
}

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
    pub balance_update1: i128,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub balance_update2: i128,
    pub exec_plan: Option<ExecutionPlanCoordinatesModel>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionPlanCoordinatesModel {
    pub sequence: u32,
    pub position: u32,
}
