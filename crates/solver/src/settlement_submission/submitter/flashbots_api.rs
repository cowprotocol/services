use {
    super::{
        super::submitter::{TransactionHandle, TransactionSubmitting},
        common::PrivateNetwork,
        Strategy,
        SubmissionLoopStatus,
    },
    crate::settlement_submission::SubmitterSettlement,
    anyhow::{Context, Result},
    ethcontract::transaction::TransactionBuilder,
    reqwest::{Client, IntoUrl},
    shared::ethrpc::{http::HttpTransport, Web3, Web3Transport},
};

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
        self.rpc
            .api::<PrivateNetwork>()
            .submit_raw_transaction(tx)
            .await
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

    fn submission_status(
        &self,
        _settlement: &SubmitterSettlement,
        _network_id: &str,
    ) -> SubmissionLoopStatus {
        SubmissionLoopStatus::Enabled
    }

    fn name(&self) -> Strategy {
        Strategy::Flashbots
    }
}
