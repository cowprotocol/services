//! Solana JSON-RPC client.

#![expect(dead_code, reason = "consumed by the on-chain orders lookup")]

use {
    solana_client::{client_error::ClientError, nonblocking::rpc_client::RpcClient},
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client_api::request::MAX_MULTIPLE_ACCOUNTS,
    solana_sdk::{account::Account, pubkey::Pubkey},
    std::{
        collections::{HashMap, HashSet},
        time::Duration,
    },
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
    /// per-request cap are split transparently.
    pub(crate) async fn multiple_accounts(
        &self,
        keys: &[Pubkey],
    ) -> Result<HashMap<Pubkey, Account>, ClientError> {
        let unique: Vec<Pubkey> = keys
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let mut accounts = HashMap::with_capacity(unique.len());
        for chunk in unique.chunks(MAX_MULTIPLE_ACCOUNTS) {
            let fetched = self.client.get_multiple_accounts(chunk).await?;
            for (key, account) in chunk.iter().zip(fetched) {
                if let Some(account) = account {
                    accounts.insert(*key, account);
                }
            }
        }
        Ok(accounts)
    }
}
