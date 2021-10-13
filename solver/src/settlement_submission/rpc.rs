use super::{
    gas_price_stream::gas_price_stream,
    retry::{CancelSender, SettlementSender},
    ESTIMATE_GAS_LIMIT_FACTOR,
};
use crate::{encoding::EncodedSettlement, pending_transactions::Fee, settlement::Settlement};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use ethcontract::{Account, TransactionHash};
use futures::stream::StreamExt;
use gas_estimation::{EstimatedGasPrice, GasPrice1559, GasPriceEstimating};
use primitive_types::{H160, U256};
use shared::Web3;
use std::time::Duration;
use transaction_retry::RetryResult;

// Submit a settlement to the contract, updating the transaction with gas prices if they increase.
#[allow(clippy::too_many_arguments)]
pub async fn submit(
    submission_nodes: &[Web3],
    account: Account,
    contract: &GPv2Settlement,
    gas: &dyn GasPriceEstimating,
    target_confirm_time: Duration,
    gas_price_cap: f64,
    settlement: Settlement,
    gas_estimate: U256,
) -> Result<TransactionHash> {
    let address = account.address();
    let settlement: EncodedSettlement = settlement.into();

    let web3 = contract.raw_instance().web3();
    let nonce = web3
        .eth()
        .transaction_count(address, None)
        .await
        .context("failed to get transaction_count")?;
    let pending_gas_price =
        recover_gas_price_from_pending_transaction(&web3, &address, nonce).await?;

    // Account for some buffer in the gas limit in case racing state changes result in slightly more heavy computation at execution time
    let gas_limit = gas_estimate.to_f64_lossy() * ESTIMATE_GAS_LIMIT_FACTOR;

    let settlement_sender = SettlementSender {
        contract,
        nodes: submission_nodes,
        nonce,
        gas_limit,
        settlement,
        account,
    };
    // We never cancel.
    let cancel_future = std::future::pending::<CancelSender>();
    if let Some(gas_price) = pending_gas_price {
        tracing::info!(
            "detected existing pending transaction with gas price {:?}",
            gas_price
        );
    }

    // It is possible that there is a pending transaction we don't know about because the driver
    // got restarted while it was in progress. Sending a new transaction could fail in that case
    // because the gas price has not increased. So we make sure that the starting gas price is at
    // least high enough to accommodate. This isn't perfect because it's still possible that that
    // transaction gets mined first in which case our new transaction would fail with "nonce already
    // used".
    let pending_gas_price =
        pending_gas_price.map(transaction_retry::gas_price_increase::minimum_increase);
    let stream = gas_price_stream(
        target_confirm_time,
        gas_price_cap,
        gas_limit,
        gas,
        pending_gas_price,
    )
    .boxed();

    match transaction_retry::retry(settlement_sender, cancel_future, stream).await {
        Some(RetryResult::Submitted(result)) => {
            tracing::info!("completed settlement submission");
            result.0.context("settlement transaction failed")
        }
        _ => unreachable!(),
    }
}

async fn recover_gas_price_from_pending_transaction(
    web3: &Web3,
    address: &H160,
    nonce: U256,
) -> Result<Option<EstimatedGasPrice>> {
    let transactions = crate::pending_transactions::pending_transactions(web3.transport())
        .await
        .context("pending_transactions failed")?;
    let transaction = match transactions
        .iter()
        .find(|transaction| transaction.from == *address && transaction.nonce == nonce)
    {
        Some(transaction) => transaction,
        None => return Ok(None),
    };
    match transaction.fee {
        Fee::Legacy { gas_price } => Ok(Some(EstimatedGasPrice {
            legacy: gas_price.to_f64_lossy(),
            ..Default::default()
        })),
        Fee::Eip1559 {
            max_priority_fee_per_gas,
            max_fee_per_gas,
        } => Ok(Some(EstimatedGasPrice {
            eip1559: Some(GasPrice1559 {
                max_fee_per_gas: max_fee_per_gas.to_f64_lossy(),
                max_priority_fee_per_gas: max_priority_fee_per_gas.to_f64_lossy(),
                base_fee_per_gas: crate::pending_transactions::base_fee_per_gas(web3.transport())
                    .await?
                    .to_f64_lossy(),
            }),
            ..Default::default()
        })),
    }
}
