//! Solana JSON-RPC client wrapper.

use {
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    std::time::Duration,
    url::Url,
};

pub struct SolanaRPC {
    inner: RpcClient,
}

impl SolanaRPC {
    /// Creates a client for the given HTTP URL and request timeout at
    /// `confirmed` commitment.
    pub fn new(url: &Url, request_timeout: Duration) -> Self {
        Self {
            inner: RpcClient::new_with_timeout_and_commitment(
                url.to_string(),
                request_timeout,
                CommitmentConfig::confirmed(),
            ),
        }
    }

    /// Returns the underlying [`RpcClient`].
    pub fn inner(&self) -> &RpcClient {
        &self.inner
    }
}

impl From<RpcClient> for SolanaRPC {
    /// Wraps an existing [`RpcClient`], for callers that need to build the
    /// underlying client with custom configuration.
    fn from(inner: RpcClient) -> Self {
        Self { inner }
    }
}
