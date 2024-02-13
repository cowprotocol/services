use {
    crate::{
        driver_logger::DriverLogger,
        settlement::Settlement,
        settlement_submission::{SolutionSubmitter, SubmissionError},
    },
    anyhow::Result,
    ethcontract::Account,
    primitive_types::U256,
    std::time::Instant,
    web3::types::TransactionReceipt,
};

pub mod gas;
pub mod solver_settlements;

/// Submits the winning solution and handles the related logging and metrics.
#[allow(clippy::too_many_arguments)]
pub async fn submit_settlement(
    solution_submitter: &SolutionSubmitter,
    logger: &DriverLogger,
    account: Account,
    nonce: U256,
    solver_name: &str,
    settlement: Settlement,
    gas_estimate: U256,
    max_fee_per_gas: f64,
    settlement_id: Option<u64>,
) -> Result<TransactionReceipt, SubmissionError> {
    let start = Instant::now();
    let result = solution_submitter
        .settle(
            settlement.clone(),
            gas_estimate,
            max_fee_per_gas,
            account,
            nonce,
        )
        .await;
    logger
        .log_submission_info(
            &result,
            &settlement,
            settlement_id,
            solver_name,
            start.elapsed(),
        )
        .await;
    result.map(Into::into)
}
