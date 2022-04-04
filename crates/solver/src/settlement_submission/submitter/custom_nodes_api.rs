use crate::{
    pending_transactions::Fee,
    settlement::{Revertable, Settlement},
};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    AdditionalTip, CancelHandle, DisabledReason, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{
    dyns::DynTransport,
    transaction::{Transaction, TransactionBuilder},
    H160, U256,
};
use futures::FutureExt;
use gas_estimation::{EstimatedGasPrice, GasPrice1559};
use shared::Web3;

const ALREADY_KNOWN_TRANSACTION: &[&str] = &[
    "Transaction gas price supplied is too low", //openethereum
    "Transaction nonce is too low",              //openethereum
    "already known",                             //infura
    "nonce too low",                             //infura
    "OldNonce",                                  //erigon
    "INTERNAL_ERROR: nonce too low",             //erigon
];

#[derive(Clone)]
pub struct CustomNodesApi {
    nodes: Vec<Web3>,
}

impl CustomNodesApi {
    pub fn new(nodes: Vec<Web3>) -> Self {
        Self { nodes }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for CustomNodesApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<DynTransport>,
    ) -> Result<TransactionHandle> {
        tracing::info!("Custom nodes submit transaction entered");
        let transaction_request = tx.build().now_or_never().unwrap().unwrap();
        let mut futures = self
            .nodes
            .iter()
            .map(|node| {
                async {
                    tracing::info!("Sending transaction...");
                    match transaction_request.clone() {
                        Transaction::Request(tx) => node.eth().send_transaction(tx).await,
                        Transaction::Raw { bytes, hash: _ } => {
                            node.eth().send_raw_transaction(bytes.0.into()).await
                        }
                    }
                }
                .boxed()
            })
            .collect::<Vec<_>>();

        loop {
            let (result, index, rest) = futures::future::select_all(futures).await;
            let lable = format!("custom_nodes_{index}");
            tracing::info!("Loop iteration with node: {}", lable);
            match result {
                Ok(tx_hash) => {
                    super::track_submission_success(lable.as_str(), true);
                    tracing::info!("created transaction with hash: {:?}", tx_hash);
                    return Ok(TransactionHandle {
                        tx_hash,
                        handle: tx_hash,
                    });
                }
                Err(err) => {
                    tracing::info!("Error on sending transaction...");
                    // error is not real error if transaction pool responded that received transaction is already in the pool
                    let real_error = match &err {
                        web3::Error::Rpc(rpc_error) => !ALREADY_KNOWN_TRANSACTION
                            .iter()
                            .any(|message| rpc_error.message.starts_with(message)),
                        _ => true,
                    };
                    if real_error {
                        tracing::warn!(?err, ?lable, "single custom node tx failed");
                        super::track_submission_success(lable.as_str(), false);
                    }
                    if rest.is_empty() {
                        return Err(anyhow::Error::from(err).context("all custom nodes tx failed"));
                    }
                    futures = rest;
                }
            }
        }
    }

    async fn cancel_transaction(&self, id: &CancelHandle) -> Result<TransactionHandle> {
        self.submit_transaction(id.noop_transaction.clone()).await
    }

    async fn recover_pending_transaction(
        &self,
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
                    base_fee_per_gas: crate::pending_transactions::base_fee_per_gas(
                        web3.transport(),
                    )
                    .await?
                    .to_f64_lossy(),
                }),
                ..Default::default()
            })),
        }
    }

    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a slightest possibility for a transaction to be reverted (check done only for mainnet)
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::HighRisk = settlement.revertable() {
                return SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::Off)
    }

    fn name(&self) -> &'static str {
        "CustomNodes"
    }
}
