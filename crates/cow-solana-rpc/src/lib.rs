//! Solana JSON-RPC client wrapper.

pub use {solana_commitment_config::CommitmentConfig, solana_rpc_client_api::client_error::Error};
use {
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{hash::Hash, signature::Signature, transaction::Transaction},
    std::time::Duration,
    url::Url,
};

pub struct SolanaRPC {
    inner: RpcClient,
}

impl SolanaRPC {
    /// Creates a client for the given HTTP URL, request timeout and
    /// commitment level.
    pub fn new_with_timeout_and_commitment(
        url: &Url,
        request_timeout: Duration,
        commitment: CommitmentConfig,
    ) -> Self {
        Self {
            inner: RpcClient::new_with_timeout_and_commitment(
                url.to_string(),
                request_timeout,
                commitment,
            ),
        }
    }

    /// Creates a client backed by a mocked sender, for tests. The `url` is
    /// interpreted as a mock directive (e.g. `"succeeds"`, `"fails"`).
    #[cfg(feature = "test-util")]
    pub fn new_mock(url: String) -> Self {
        Self {
            inner: RpcClient::new_mock(url),
        }
    }

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.inner.get_slot().await
    }

    /// A recent blockhash for transaction signing.
    pub async fn latest_blockhash(&self) -> Result<Hash, Error> {
        self.inner.get_latest_blockhash().await
    }

    /// Submit a signed transaction without waiting for confirmation.
    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<Signature, Error> {
        self.inner.send_transaction(transaction).await
    }
}
