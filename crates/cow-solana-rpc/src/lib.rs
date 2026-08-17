//! Solana JSON-RPC client wrapper.

pub use solana_commitment_config::CommitmentConfig;
use {solana_rpc_client::nonblocking::rpc_client::RpcClient, std::time::Duration, url::Url};

pub struct SolanaRPC {
    /// Not yet read: helper methods that use the underlying client will be
    /// added in follow-up PRs.
    #[expect(dead_code)]
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
}
