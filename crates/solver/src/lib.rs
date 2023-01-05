mod analytics;
pub mod arguments;
mod auction_preprocessing;
pub mod driver;
pub mod driver_logger;
pub mod encoding;
pub mod in_flight_orders;
pub mod interactions;
pub mod liquidity;
pub mod liquidity_collector;
pub mod metrics;
pub mod orderbook;
pub mod s3_instance_upload;
pub mod s3_instance_upload_arguments;
pub mod settlement;
pub mod settlement_access_list;
pub mod settlement_post_processing;
pub mod settlement_ranker;
pub mod settlement_rater;
pub mod settlement_simulation;
pub mod settlement_submission;
pub mod solver;
#[cfg(test)]
mod test;

use anyhow::Result;
use shared::ethrpc::Web3;

pub async fn get_settlement_contract(web3: &Web3) -> Result<contracts::GPv2Settlement> {
    Ok(contracts::GPv2Settlement::deployed(web3).await?)
}
