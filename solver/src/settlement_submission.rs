pub mod archerapi;
mod gas_price_stream;
pub mod public_mempool;
pub mod retry;

use crate::encoding::EncodedSettlement;
use contracts::GPv2Settlement;
use ethcontract::errors::ExecutionError;
use primitive_types::U256;
use std::time::Duration;

const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;
const GAS_PRICE_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub async fn estimate_gas(
    contract: &GPv2Settlement,
    settlement: &EncodedSettlement,
) -> Result<U256, ExecutionError> {
    retry::settle_method_builder(contract, settlement.clone())
        .tx
        .estimate_gas()
        .await
}
