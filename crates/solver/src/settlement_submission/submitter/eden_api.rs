//! https://docs.edennetwork.io/for-traders/getting-started

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    DisabledReason, SubmissionLoopStatus,
};
use anyhow::Result;
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder};
use gas_estimation::EstimatedGasPrice;
use reqwest::Client;

const URL: &str = "https://api.edennetwork.io/v1/rpc";

#[derive(Clone)]
pub struct EdenApi {
    client: Client,
}

impl EdenApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for EdenApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<DynTransport>,
    ) -> Result<TransactionHandle, SubmitApiError> {
        super::common::submit_raw_transaction(self.client.clone(), URL, tx).await
    }

    async fn cancel_transaction(&self, _id: &TransactionHandle) -> Result<()> {
        Ok(())
    }

    fn submission_status(&self, gas_price: &EstimatedGasPrice) -> SubmissionLoopStatus {
        if gas_price.effective_gas_price() < 500. { //500 as argument?
            SubmissionLoopStatus::Enabled
        } else {
            SubmissionLoopStatus::Disabled(DisabledReason::EdenDisabledNetworkCongested)
        }
    }
}
