//! https://docs.edennetwork.io/for-traders/getting-started

use super::super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting};
use anyhow::Result;
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder};
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
}
