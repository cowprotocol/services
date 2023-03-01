use {
    super::RawTransaction,
    anyhow::Result,
    shared::ethrpc::{Web3, Web3Transport},
    web3::{api::Namespace, types::Bytes},
};

/// An additonal specialized submitter API for private network transactions.
#[derive(Clone)]
pub struct PrivateNetwork(Web3);

impl Namespace<Web3Transport> for PrivateNetwork {
    fn new(transport: Web3Transport) -> Self {
        Self(Web3::new(transport))
    }

    fn transport(&self) -> &Web3Transport {
        self.0.transport()
    }
}

impl PrivateNetwork {
    /// Function for sending raw signed transaction to private networks
    pub async fn submit_raw_transaction(&self, tx: RawTransaction) -> Result<()> {
        self.0
            .eth()
            .send_raw_transaction(Bytes(tx.0))
            .await
            .map(|_| ())
            .map_err(Into::into)
    }
}
