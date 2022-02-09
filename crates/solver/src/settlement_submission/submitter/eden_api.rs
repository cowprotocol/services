//! https://docs.edennetwork.io/for-traders/getting-started

use crate::settlement::Settlement;

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    CancelHandle, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder, H160, U256};
use gas_estimation::EstimatedGasPrice;
use reqwest::{Client, IntoUrl, Url};
use shared::Web3;

#[derive(Clone)]
pub struct EdenApi {
    client: Client,
    url: Url,
}

impl EdenApi {
    pub fn new(client: Client, url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client,
            url: url.into_url().context("bad eden url")?,
        })
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for EdenApi {
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
        super::common::submit_raw_transaction(
            self.client.clone(),
            self.url.clone(),
            id.noop_transaction.clone(),
        )
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

    fn submission_status(
        &self,
        _settlement: &Settlement,
        _network_id: &str,
    ) -> SubmissionLoopStatus {
        SubmissionLoopStatus::Enabled
    }
}
