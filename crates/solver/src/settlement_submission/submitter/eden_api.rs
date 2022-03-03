//! https://docs.edennetwork.io/for-traders/getting-started

use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    common::PrivateNetwork,
    AdditionalTip, CancelHandle, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{
    transaction::{Transaction, TransactionBuilder},
    Bytes, H160, H256, U256,
};
use futures::{FutureExt, TryFutureExt};
use gas_estimation::EstimatedGasPrice;
use reqwest::{Client, IntoUrl};
use serde::Deserialize;
use shared::{transport::http::HttpTransport, Web3, Web3Transport};
use web3::Transport;

#[derive(Clone)]
pub struct EdenApi {
    rpc: Web3,
}

#[derive(Debug, Clone, Deserialize)]
struct EdenSuccess {
    result: H256,
}

impl EdenApi {
    pub fn new(client: Client, url: impl IntoUrl) -> Result<Self> {
        let transport = Web3Transport::new(HttpTransport::new(
            client,
            url.into_url().context("bad eden url")?,
            "eden".to_owned(),
        ));
        let rpc = Web3::new(transport);

        Ok(Self { rpc })
    }

    async fn submit_slot_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle, SubmitApiError> {
        let (raw_signed_transaction, tx_hash) = match tx.build().now_or_never().unwrap().unwrap() {
            Transaction::Request(_) => unreachable!("verified offline account was used"),
            Transaction::Raw { bytes, hash } => (bytes.0, hash),
        };
        let params =
            serde_json::to_value(Bytes(raw_signed_transaction)).context("failed to serialize")?;

        let response = self
            .rpc
            .transport()
            .execute("eth_sendSlotTx", vec![params])
            .await
            .context("transport failed")?;
        tracing::debug!("response from eden: {:?}", response);

        let success =
            serde_json::from_value::<EdenSuccess>(response).context("deserialize failed")?;
        Ok(TransactionHandle {
            tx_hash,
            handle: success.result,
        })
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for EdenApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle, SubmitApiError> {
        // try to submit with slot method
        self.submit_slot_transaction(tx.clone())
            .or_else(|err| async move {
                // fallback to standard eth_sendRawTransaction
                // want to keep this call as a safety measure until we are confident in `submit_slot_transaction`
                tracing::debug!("fallback to eth_sendRawTransaction with error {:?}", err);
                self.rpc
                    .api::<PrivateNetwork>()
                    .submit_raw_transaction(tx)
                    .await
            })
            .await
    }

    async fn cancel_transaction(
        &self,
        id: &CancelHandle,
    ) -> Result<TransactionHandle, SubmitApiError> {
        self.rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(id.noop_transaction.clone())
            .await
    }

    async fn recover_pending_transaction(
        &self,
        _web3: &Web3,
        _address: &H160,
        _nonce: U256,
    ) -> Result<Option<EstimatedGasPrice>> {
        Ok(None)
    }

    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a high possibility for a transaction to be reverted (check done only for mainnet)
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::NoRisk = settlement.revertable() {
                return SubmissionLoopStatus::Enabled(AdditionalTip::Off);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::On)
    }

    fn name(&self) -> &'static str {
        "Eden"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn eden_success_response() {
        let response = serde_json::json!({
            "code": 200i64,
            "result": "0x41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31",
        });
        let deserialized = serde_json::from_value::<EdenSuccess>(response).unwrap();
        assert_eq!(
            deserialized.result,
            H256::from_str("41df922fd0d4766fcc02e161f8295ec28522f329ae487f14d811e4b64c8d6e31")
                .unwrap()
        );
    }
}
