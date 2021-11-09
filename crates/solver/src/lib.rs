mod analytics;
pub mod driver;
pub mod encoding;
pub mod in_flight_orders;
pub mod interactions;
pub mod liquidity;
pub mod liquidity_collector;
pub mod metrics;
pub mod orderbook;
pub mod pending_transactions;
pub mod settlement;
pub mod settlement_simulation;
pub mod settlement_submission;
pub mod solver;
#[cfg(test)]
mod test;

use anyhow::Result;
use shared::Web3;

pub async fn get_settlement_contract(web3: &Web3) -> Result<contracts::GPv2Settlement> {
    Ok(contracts::GPv2Settlement::deployed(web3).await?)
}
