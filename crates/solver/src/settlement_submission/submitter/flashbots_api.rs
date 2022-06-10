use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    common::PrivateNetwork,
    AdditionalTip, Strategy, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::transaction::TransactionBuilder;
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
    ) -> Result<TransactionHandle> {
        let result = self
            .rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(tx)
            .await;

        super::track_submission_success("flashbots", result.is_ok());

        result
    }

    // https://docs.flashbots.net/flashbots-protect/rpc/cancellations
    async fn cancel_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        self.rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(tx)
            .await
    }

    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus {
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::NoRisk = settlement.revertable() {
                return SubmissionLoopStatus::Enabled(AdditionalTip::Off);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::On)
    }

    fn name(&self) -> Strategy {
        Strategy::Flashbots
    }
}
