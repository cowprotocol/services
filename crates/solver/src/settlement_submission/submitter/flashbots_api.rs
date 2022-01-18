use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    CancelHandle,
};
use anyhow::{Context, Result};
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder};
use reqwest::{Client, IntoUrl, Url};

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

    async fn cancel_transaction(&self, _id: &CancelHandle) -> Result<()> {
        Ok(())
    }

    async fn mark_transaction_outdated(&self, _id: &TransactionHandle) -> Result<()> {
        Ok(())
    }
}
