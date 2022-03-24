use super::super::submitter::TransactionHandle;
use anyhow::Result;
use ethcontract::transaction::{Transaction, TransactionBuilder};
use futures::FutureExt;
use shared::{Web3, Web3Transport};
use web3::{api::Namespace, types::Bytes};

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
    pub async fn submit_raw_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        let (raw_signed_transaction, tx_hash) = match tx.build().now_or_never().unwrap().unwrap() {
            Transaction::Request(_) => unreachable!("verified offline account was used"),
            Transaction::Raw { bytes, hash } => (bytes.0, hash),
        };

        let handle = self
            .0
            .eth()
            .send_raw_transaction(Bytes(raw_signed_transaction))
            .await
            .map_err(anyhow::Error::new)?;

        Ok(TransactionHandle { tx_hash, handle })
    }
}
