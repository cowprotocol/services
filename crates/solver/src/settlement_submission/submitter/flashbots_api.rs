use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    common::PrivateNetwork,
    AdditionalTip, CancelHandle, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{transaction::TransactionBuilder, H160, U256};
use gas_estimation::EstimatedGasPrice;
use reqwest::{Client, IntoUrl};
use shared::{transport::http::HttpTransport, Web3, Web3Transport};

#[derive(Clone)]
pub struct FlashbotsApi {
    rpc: Web3,
}

impl FlashbotsApi {
    pub fn new(client: Client, url: impl IntoUrl) -> Result<Self> {
        let transport = Web3Transport::new(HttpTransport::new(
            client,
            url.into_url().context("bad flashbots url")?,
            "flashbots".to_owned(),
        ));
        let rpc = Web3::new(transport);

        Ok(Self { rpc })
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for FlashbotsApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle, SubmitApiError> {
        self.rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(tx)
            .await
    }

    // https://docs.flashbots.net/flashbots-protect/rpc/cancellations
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
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::NoRisk = settlement.revertable() {
                return SubmissionLoopStatus::Enabled(AdditionalTip::Off);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::On)
    }

    fn name(&self) -> &'static str {
        "Flashbots"
    }
}
