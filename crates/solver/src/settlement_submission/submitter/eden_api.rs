//! https://docs.edennetwork.io/for-traders/getting-started

use super::{
    super::submitter::{SubmitApiError, TransactionHandle, TransactionSubmitting},
    CancelHandle,
};
use anyhow::{anyhow, Context, Result};
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder};
use reqwest::{Client, IntoUrl, Url};

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

    async fn cancel_transaction(&self, id: &CancelHandle) -> Result<()> {
        match super::common::submit_raw_transaction(
            self.client.clone(),
            self.url.clone(),
            id.noop_transaction.clone(),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow!("{:?}", err)),
        }
    }

    async fn mark_transaction_outdated(&self, _id: &TransactionHandle) -> Result<()> {
        Ok(())
    }
}
