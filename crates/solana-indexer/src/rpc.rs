//! Solana JSON-RPC client.

#![expect(dead_code, reason = "consumed by the on-chain orders lookup")]

use {
    solana_client::{client_error::ClientError, nonblocking::rpc_client::RpcClient},
    solana_commitment_config::CommitmentConfig,
    solana_sdk::{account::Account, pubkey::Pubkey},
    std::time::Duration,
    url::Url,
};

/// The server rejects `getMultipleAccounts` batches above this size.
const MAX_ACCOUNTS_PER_REQUEST: usize = 100;

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

    /// Fetch accounts by key, `None` for keys that do not exist. Batches
    /// above the server's per-request cap are split transparently.
    pub(crate) async fn multiple_accounts(
        &self,
        keys: &[Pubkey],
    ) -> Result<Vec<Option<Account>>, ClientError> {
        let mut accounts = Vec::with_capacity(keys.len());
        for chunk in keys.chunks(MAX_ACCOUNTS_PER_REQUEST) {
            accounts.extend(self.client.get_multiple_accounts(chunk).await?);
        }
        Ok(accounts)
    }
}
