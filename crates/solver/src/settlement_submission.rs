pub mod archer_api;
pub mod archer_settlement;
mod dry_run;
pub mod flashbots_api;
pub mod flashbots_settlement;
mod gas_price_stream;
pub mod retry;
pub mod rpc;

use crate::settlement::Settlement;
use anyhow::{bail, Result};
use archer_api::ArcherApi;
use contracts::GPv2Settlement;
use ethcontract::Account;
use flashbots_api::FlashbotsApi;
use gas_estimation::GasPriceEstimating;
use primitive_types::U256;
use shared::Web3;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use web3::types::TransactionReceipt;

use self::{
    archer_settlement::ArcherSolutionSubmitter, flashbots_settlement::FlashbotsSolutionSubmitter,
};

const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;
const GAS_PRICE_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub struct SolutionSubmitter {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
    // for gas price estimation
    pub target_confirm_time: Duration,
    pub gas_price_cap: f64,
    pub transaction_strategy: TransactionStrategy,
}

pub enum TransactionStrategy {
    ArcherNetwork {
        archer_api: ArcherApi,
        max_confirm_time: Duration,
    },
    Flashbots {
        flashbots_api: FlashbotsApi,
        max_confirm_time: Duration,
        flashbots_tip: f64,
    },
    CustomNodes(Vec<Web3>),
    DryRun,
}

impl SolutionSubmitter {
    /// Submits a settlement transaction to the blockchain, returning the hash
    /// of the successfully mined transaction.
    ///
    /// Errors if the transaction timed out, or an inner error was encountered
    /// during submission.
    pub async fn settle(
        &self,
        settlement: Settlement,
        gas_estimate: U256,
        account: Account,
    ) -> Result<TransactionReceipt> {
        match &self.transaction_strategy {
            TransactionStrategy::CustomNodes(nodes) => {
                rpc::submit(
                    nodes,
                    account,
                    &self.contract,
                    self.gas_price_estimator.as_ref(),
                    self.target_confirm_time,
                    self.gas_price_cap,
                    settlement,
                    gas_estimate,
                )
                .await
            }
            TransactionStrategy::ArcherNetwork {
                archer_api,
                max_confirm_time,
            } => {
                let submitter = ArcherSolutionSubmitter::new(
                    &self.web3,
                    &self.contract,
                    &account,
                    archer_api,
                    self.gas_price_estimator.as_ref(),
                    self.gas_price_cap,
                )?;
                let result = submitter
                    .submit(
                        self.target_confirm_time,
                        SystemTime::now() + *max_confirm_time,
                        settlement,
                        gas_estimate,
                    )
                    .await;
                match result {
                    Ok(Some(hash)) => Ok(hash),
                    Ok(None) => bail!("transaction did not get mined in time"),
                    Err(err) => Err(err),
                }
            }
            TransactionStrategy::Flashbots {
                flashbots_api,
                max_confirm_time,
                flashbots_tip,
            } => {
                let submitter = FlashbotsSolutionSubmitter::new(
                    &self.web3,
                    &self.contract,
                    &account,
                    flashbots_api,
                    self.gas_price_estimator.as_ref(),
                    self.gas_price_cap,
                )?;
                let result = submitter
                    .submit(
                        self.target_confirm_time,
                        SystemTime::now() + *max_confirm_time,
                        settlement,
                        gas_estimate,
                        *flashbots_tip,
                    )
                    .await;
                match result {
                    Ok(Some(hash)) => Ok(hash),
                    Ok(None) => bail!("transaction did not get mined in time"),
                    Err(err) => Err(err),
                }
            }
            TransactionStrategy::DryRun => {
                dry_run::log_settlement(account, &self.contract, settlement).await
            }
        }
    }
}
