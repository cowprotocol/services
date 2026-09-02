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

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.inner.get_slot().await
    }

    /// The current block height at the client's commitment level.
    pub async fn block_height(&self) -> Result<BlockHeight, Error> {
        Ok(BlockHeight(self.inner.get_block_height().await?))
    }

    /// The latest confirmed blockhash and the last block height at
    /// which it stays usable.
    ///
    /// The method always fetches the blockhash at `confirmed`,
    /// regardless of the client's configured commitment level.
    pub async fn latest_confirmed_blockhash(&self) -> Result<LatestBlockhash, Error> {
        let (blockhash, last_valid_block_height) = self
            .inner
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .await?;
        Ok(LatestBlockhash {
            blockhash,
            last_valid_block_height: BlockHeight(last_valid_block_height),
        })
    }

    /// Send a versioned transaction and wait until it reaches the client's
    /// configured commitment level.
    ///
    /// Each individual RPC request is capped at the client's request timeout,
    /// but the confirm loop has no overall timeout: it polls until the
    /// transaction confirms or its blockhash expires.
    pub async fn send_and_confirm_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.inner.send_and_confirm_transaction(transaction).await
    }
}

/// A Solana block height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockHeight(u64);

impl BlockHeight {
    /// The height as a plain number.
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<u64> for BlockHeight {
    fn from(height: u64) -> Self {
        Self(height)
    }
}

impl From<BlockHeight> for u64 {
    fn from(height: BlockHeight) -> Self {
        height.0
    }
}

/// The latest confirmed blockhash and the last block height at which
/// it stays usable.
#[derive(Debug)]
pub struct LatestBlockhash {
    /// The blockhash to sign transactions with.
    pub blockhash: Hash,
    /// The last block height at which transactions signed with this
    /// blockhash as `recent_blockhash` remain valid.
    pub last_valid_block_height: BlockHeight,
}
