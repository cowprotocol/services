#![expect(dead_code)]
//! Solana RPC interface for recovery.

use {
    crate::types::{
        commitment::{AccountInfo, SignatureStatus},
        recovery::GetSignaturesOpts,
        wire::SubscribeUpdateTransactionInfo,
    },
    async_trait::async_trait,
    solana_client::client_error::ClientError,
    solana_sdk::{pubkey::Pubkey, signature::Signature},
};

/// Interface for the RPC calls recovery needs: re-fetching dead-lettered
/// transactions by signature, auditing unfinalized rows for fork rollbacks,
/// and reading account state.
#[async_trait]
pub(crate) trait SolanaClient: Send + Sync {
    /// Fetch status for multiple transaction signatures (up to 256).
    /// `None` = transaction signature not found.
    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<SignatureStatus>>, ClientError>;

    /// Fetch a transaction by its signature. `Ok(None)` = never landed.
    async fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<Option<SubscribeUpdateTransactionInfo>, ClientError>;

    /// List all transaction signatures for a program address (used for
    /// backfill).
    async fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        opts: GetSignaturesOpts,
    ) -> Result<Vec<Signature>, ClientError>;

    /// Read account data. `Ok(None)` = account does not exist (deleted or not
    /// initialized).
    async fn get_account_info(&self, address: &Pubkey) -> Result<Option<AccountInfo>, ClientError>;
}
