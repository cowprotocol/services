use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    AdditionalTip, CancelHandle, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder, H160, U256};
use gas_estimation::EstimatedGasPrice;
use reqwest::{Client, IntoUrl, Url};
use shared::Web3;

#[derive(Clone)]
pub struct FlashbotsApi {
    client: Client,
    url: Url,
}

impl FlashbotsApi {
    pub fn new(client: Client, url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client,
            url: url.into_url().context("bad flashbots url")?,
        })
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for FlashbotsApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<DynTransport>,
    ) -> Result<TransactionHandle, SubmitApiError> {
        super::common::submit_raw_transaction(self.client.clone(), self.url.clone(), tx).await
    }

    async fn cancel_transaction(
        &self,
        id: &CancelHandle,
    ) -> Result<TransactionHandle, SubmitApiError> {
        Ok(id.submitted_transaction)
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
