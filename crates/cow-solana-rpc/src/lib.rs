//! Solana JSON-RPC client wrapper.

use {
    futures::{TryFutureExt, future},
    itertools::Itertools,
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_rpc_client_api::request::MAX_MULTIPLE_ACCOUNTS,
    solana_sdk::{account::Account, pubkey::Pubkey},
    std::{collections::HashMap, time::Duration},
    url::Url,
};
pub use {solana_commitment_config::CommitmentConfig, solana_rpc_client_api::client_error::Error};
#[cfg(feature = "test-util")]
pub use {solana_rpc_client::mock_sender::Mocks, solana_rpc_client_api::request::RpcRequest};

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

    /// Creates a client answering from canned per-request responses, for
    /// tests.
    #[cfg(feature = "test-util")]
    pub fn new_mock_with_mocks(mocks: Mocks) -> Self {
        Self {
            inner: RpcClient::new_mock_with_mocks("mock".to_owned(), mocks),
        }
    }

    /// Fetch accounts by key. Accounts that do not exist are absent from the
    /// map. Duplicate keys are fetched once, and batches above the server's
    /// per-request cap are split into parallel requests.
    pub async fn multiple_accounts(
        &self,
        keys: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Account>, Error> {
        let unique: Vec<Pubkey> = keys.into_iter().unique().collect();
        let fetched = future::try_join_all(unique.chunks(MAX_MULTIPLE_ACCOUNTS).map(|chunk| {
            self.inner
                .get_multiple_accounts(chunk)
                .map_ok(move |accounts| {
                    accounts
                        .into_iter()
                        .zip(chunk)
                        .filter_map(|(account, key)| Some((*key, account?)))
                        .collect::<Vec<_>>()
                })
        }))
        .await?;
        Ok(fetched.into_iter().flatten().collect())
    }
}
