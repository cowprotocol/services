use super::super::submitter::{SubmitApiError, TransactionHandle};
use ethcontract::{
    jsonrpc::types::error::Error as RpcError,
    transaction::{Transaction, TransactionBuilder},
};
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
    ) -> Result<TransactionHandle, SubmitApiError> {
        let (raw_signed_transaction, tx_hash) = match tx.build().now_or_never().unwrap().unwrap() {
            Transaction::Request(_) => unreachable!("verified offline account was used"),
            Transaction::Raw { bytes, hash } => (bytes.0, hash),
        };

        let handle = self
            .0
            .eth()
            .send_raw_transaction(Bytes(raw_signed_transaction))
            .await
            .map_err(convert_web3_to_submission_error)?;

        Ok(TransactionHandle { tx_hash, handle })
    }
}

fn convert_web3_to_submission_error(err: web3::Error) -> SubmitApiError {
    if let web3::Error::Rpc(RpcError { message, .. }) = &err {
        if message.starts_with("invalid nonce") || message.starts_with("nonce too low") {
            return SubmitApiError::InvalidNonce;
        } else if message.starts_with("Transaction gas price supplied is too low") {
            return SubmitApiError::OpenEthereumTooCheapToReplace;
        } else if message.starts_with("replacement transaction underpriced") {
            return SubmitApiError::ReplacementTransactionUnderpriced;
        } else if message.contains("tx fee") && message.contains("exceeds the configured cap") {
            return SubmitApiError::EdenTransactionTooExpensive;
        }
    }

    anyhow::Error::new(err)
        .context("transaction submission error")
        .into()
}
