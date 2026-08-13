//! Solana JSON-RPC client.

#![expect(dead_code, reason = "consumed by the on-chain orders lookup")]

use {
    futures::future,
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
    pub(crate) fn new(endpoint: Url, request_timeout: Duration) -> Self {
        Self {
            client: RpcClient::new_with_timeout_and_commitment(
                String::from(endpoint),
                request_timeout,
                CommitmentConfig::confirmed(),
            ),
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
        let fetched = future::try_join_all(
            unique
                .chunks(MAX_MULTIPLE_ACCOUNTS)
                .map(|chunk| self.client.get_multiple_accounts(chunk)),
        )
        .await?;
        Ok(unique
            .iter()
            .zip(fetched.into_iter().flatten())
            .filter_map(|(key, account)| Some((*key, account?)))
            .collect())
    }
}
