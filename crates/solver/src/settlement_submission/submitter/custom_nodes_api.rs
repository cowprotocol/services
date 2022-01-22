use crate::pending_transactions::Fee;

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    CancelHandle,
};
use anyhow::{anyhow, Context, Result};
use ethcontract::{
    dyns::DynTransport,
    transaction::{Transaction, TransactionBuilder},
    H160, U256,
};
use futures::FutureExt;
use gas_estimation::{EstimatedGasPrice, GasPrice1559};
use shared::Web3;

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
    ) -> Result<TransactionHandle, SubmitApiError> {
        tracing::info!("sending transaction to custom nodes...");
        let transaction_request = tx.build().now_or_never().unwrap().unwrap();
        let mut futures = self
            .nodes
            .iter()
            .map(|node| {
                async {
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
            let (result, _index, rest) = futures::future::select_all(futures).await;
            match result {
                Ok(tx_hash) => {
                    tracing::info!("created transaction with hash: {}", tx_hash);
                    return Ok(TransactionHandle {
                        tx_hash,
                        handle: tx_hash,
                    });
                }
                Err(err) if rest.is_empty() => {
                    tracing::debug!("error {}", err);
                    return Err(anyhow::Error::from(err)
                        .context("all nodes tx failed")
                        .into());
                }
                Err(err) => {
                    tracing::warn!(?err, "single node tx failed");
                    futures = rest;
                }
            }
        }
    }

    async fn cancel_transaction(&self, id: &CancelHandle) -> Result<()> {
        match self.submit_transaction(id.noop_transaction.clone()).await {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow!("{:?}", err)),
        }
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
}
