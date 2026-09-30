//! The Solana blockchain adapter.
//!
//! Owns the RPC client and the settlement program id. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

mod accounts;
mod token;

pub use {
    accounts::{AccountsSnapshot, InvalidAddressLookupTableReason, TokenAccountState},
    token::{
        associated_token_address,
        close_token_account,
        create_associated_token_account_idempotent,
        require_token_balance,
    },
};
use {
    cow_solana_rpc::{Error, LatestBlockhash, SolanaRPC},
    solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction},
};

/// The Solana blockchain adapter.
pub struct Solana {
    rpc: SolanaRPC,
    /// Serves `simulateBundle` instead of `rpc` when set.
    bundle_rpc: Option<SolanaRPC>,
    program_id: Pubkey,
}

impl Solana {
    /// Build the adapter from the RPC client and the settlement program id.
    pub fn new(rpc: SolanaRPC, program_id: Pubkey) -> Self {
        Self {
            rpc,
            bundle_rpc: None,
            program_id,
        }
    }

    /// Route `simulateBundle` to a dedicated client.
    pub fn with_bundle_rpc(mut self, rpc: SolanaRPC) -> Self {
        self.bundle_rpc = Some(rpc);
        self
    }

    /// The settlement program id this driver settles against.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// Fetch the latest confirmed blockhash and the last block height at
    /// which it stays usable.
    pub async fn latest_confirmed_blockhash(&self) -> Result<LatestBlockhash, Error> {
        self.rpc.latest_confirmed_blockhash().await
    }

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.rpc.slot().await
    }

    /// Simulate a signed transaction without sending it. Returns the
    /// simulation result including logs and any error.
    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<cow_solana_rpc::RpcSimulateTransactionResult, Error> {
        self.rpc.simulate_transaction(transaction).await
    }

    /// See [`SolanaRPC::simulate_bundle`].
    pub async fn simulate_bundle(
        &self,
        transactions: &[VersionedTransaction],
    ) -> Result<Vec<cow_solana_rpc::RpcSimulateBundleTransactionResult>, Error> {
        self.bundle_rpc
            .as_ref()
            .unwrap_or(&self.rpc)
            .simulate_bundle(transactions)
            .await
    }

    /// Send a signed transaction and wait for confirmation.
    pub async fn send_and_confirm_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.rpc.send_and_confirm_transaction(transaction).await
    }

    /// Whether each signature is known to the cluster, within the node's
    /// transaction-history horizon.
    pub async fn known_signatures(&self, signatures: &[Signature]) -> Result<Vec<bool>, Error> {
        self.rpc.known_signatures(signatures).await
    }

    /// Fetch the accounts at `keys` in a single batched fetch (split into
    /// parallel requests above the server's per-request cap) and return them
    /// as a snapshot ready for typed interpretation.
    pub async fn accounts_snapshot(
        &self,
        keys: impl IntoIterator<Item = Pubkey>,
    ) -> Result<AccountsSnapshot, Error> {
        Ok(AccountsSnapshot::new(
            self.rpc.multiple_accounts(keys).await?,
        ))
    }
}
