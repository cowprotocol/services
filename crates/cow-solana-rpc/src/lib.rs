//! Solana JSON-RPC client wrapper.

use {
    futures::{TryFutureExt, future},
    itertools::Itertools,
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_rpc_client_api::request::MAX_MULTIPLE_ACCOUNTS,
    solana_sdk::{
        account::Account,
        hash::Hash,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::{collections::HashMap, time::Duration},
    url::Url,
};
pub use {
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client_api::{
        client_error::Error,
        response::{RpcSimulateTransactionResult, UiTransactionError},
    },
};
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

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.inner.get_slot().await
    }

    /// The latest blockhash and the last block height at which it remains valid
    /// (so consumers know whether the blockhash is still usable), fetched at
    /// the client's configured commitment level.
    ///
    /// Note: this uses the same commitment level the client was configured
    /// with. It's important to consider that a `processed` blockhash may come
    /// from an abandoned fork, a `finalized` blockhash comes at the expense of
    /// shortening the transaction's ~150-block validity window. `confirmed` is
    /// usually the safest default.
    pub async fn latest_blockhash(&self) -> Result<(Hash, u64), Error> {
        self.inner
            .get_latest_blockhash_with_commitment(self.inner.commitment())
            .await
    }

    /// Send a versioned transaction and wait until it reaches the client's
    /// configured commitment level.
    pub async fn send_and_confirm_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.inner.send_and_confirm_transaction(transaction).await
    }

    /// Simulate a versioned transaction without sending it. Returns the
    /// simulation result including logs and any error.
    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<solana_rpc_client_api::response::RpcSimulateTransactionResult, Error> {
        self.inner
            .simulate_transaction(transaction)
            .await
            .map(|response| response.value)
    }
}
