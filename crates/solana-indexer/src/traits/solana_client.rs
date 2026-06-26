#![expect(dead_code)]
//! Solana RPC interface for the finalization worker.

use {
    crate::types::{
        commitment::{AccountInfo, SignatureStatus},
        recovery::GetSignaturesOpts,
        wire::SubscribeUpdateTransactionInfo,
    },
    solana_client::client_error::ClientError,
    solana_sdk::{pubkey::Pubkey, signature::Signature},
};

/// Interface for RPC calls the finalization worker needs:
/// promoting confirmed transactions to finalized, sweeping aged rows,
/// and reading account state for recovery.
pub(crate) trait SolanaClient {
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
