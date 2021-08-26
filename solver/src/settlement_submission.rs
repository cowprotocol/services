pub mod archer_api;
pub mod archer_settlement;
mod gas_price_stream;
pub mod public_mempool;
pub mod retry;

use crate::{encoding::EncodedSettlement, settlement::Settlement};
use anyhow::{anyhow, Result};
use archer_api::ArcherApi;
use contracts::GPv2Settlement;
use ethcontract::{errors::ExecutionError, Account, TransactionHash};
use gas_estimation::GasPriceEstimating;
use primitive_types::U256;
use shared::Web3;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use self::archer_settlement::ArcherSolutionSubmitter;

const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;
const GAS_PRICE_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub async fn estimate_gas(
    contract: &GPv2Settlement,
    settlement: &EncodedSettlement,
    from: Account,
) -> Result<U256, ExecutionError> {
    retry::settle_method_builder(contract, settlement.clone(), from)
        .tx
        .estimate_gas()
        .await
}

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
    PublicMempool,
    ArcherNetwork {
        archer_api: ArcherApi,
        max_confirm_time: Duration,
    },
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
    ) -> Result<TransactionHash> {
        match &self.transaction_strategy {
            TransactionStrategy::PublicMempool => {
                public_mempool::submit(
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
                let submitter = ArcherSolutionSubmitter {
                    web3: &self.web3,
                    contract: &self.contract,
                    account: &account,
                    archer_api,
                    gas_price_estimator: self.gas_price_estimator.as_ref(),
                    gas_price_cap: self.gas_price_cap,
                };
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
                    Ok(None) => Err(anyhow!("transaction did not get mined in time")),
                    Err(err) => Err(err),
                }
            }
        }
    }
}
