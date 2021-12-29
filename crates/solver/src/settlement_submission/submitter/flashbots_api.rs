use super::super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting};
use anyhow::Result;
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder};
use reqwest::Client;

const URL: &str = "https://rpc.flashbots.net";

#[derive(Clone)]
pub struct FlashbotsApi {
    client: Client,
}

impl FlashbotsApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for FlashbotsApi {
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
