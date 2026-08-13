//! Solana JSON-RPC client.

use {
    futures::{TryFutureExt, future},
    itertools::Itertools,
    solana_client::{client_error::ClientError, nonblocking::rpc_client::RpcClient},
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client_api::request::MAX_MULTIPLE_ACCOUNTS,
    solana_sdk::{account::Account, pubkey::Pubkey},
    std::{collections::HashMap, time::Duration},
    url::Url,
};

/// JSON-RPC client over the indexer's RPC endpoint, at `confirmed`
/// commitment to match the stream subscription.
pub(crate) struct Rpc {
    client: RpcClient,
}

impl Rpc {
    #[expect(dead_code, reason = "constructed by the binary wiring")]
    pub(crate) fn new(endpoint: Url, request_timeout: Duration) -> Self {
        Self {
            client: RpcClient::new_with_timeout_and_commitment(
                String::from(endpoint),
                request_timeout,
                CommitmentConfig::confirmed(),
            ),
        }
    }

    /// A client over a canned transport, for tests.
    #[cfg(test)]
    pub(crate) fn new_mock(mocks: solana_client::rpc_client::Mocks) -> Self {
        Self {
            client: RpcClient::new_mock_with_mocks("mock".to_owned(), mocks),
        }
    }

    /// Fetch accounts by key. Accounts that do not exist are absent from the
    /// map. Duplicate keys are fetched once, and batches above the server's
    /// per-request cap are split into parallel requests.
    pub(crate) async fn multiple_accounts(
        &self,
        keys: &[Pubkey],
    ) -> Result<HashMap<Pubkey, Account>, ClientError> {
        let unique: Vec<Pubkey> = keys.iter().copied().unique().collect();
        let fetched = future::try_join_all(unique.chunks(MAX_MULTIPLE_ACCOUNTS).map(|chunk| {
            self.client
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
