//! The Solana blockchain adapter.
//!
//! Owns the RPC client, the settlement program id and the token program of
//! each mint looked up so far. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

mod accounts;
mod token;

pub use {
    accounts::{
        AccountsSnapshot,
        InvalidAddressLookupTableReason,
        InvalidMintReason,
        TokenAccountState,
    },
    token::{
        associated_token_address,
        close_token_account,
        create_associated_token_account_idempotent,
        require_token_balance,
    },
};
use {
    cow_settlement_interface::token_program::TokenProgram,
    cow_solana_rpc::{Error, LatestBlockhash, SolanaRPC},
    itertools::{Either, Itertools},
    moka::sync::Cache,
    solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction},
    std::collections::HashMap,
};

/// How many mints the token program cache holds.
const TOKEN_PROGRAM_CACHE_CAPACITY: u64 = 10_000;

/// The Solana blockchain adapter.
pub struct Solana {
    rpc: SolanaRPC,
    program_id: Pubkey,
    /// The token program of each mint looked up so far. Entries never expire:
    /// a mint's owner changes only if the mint is closed and re-created.
    token_programs: Cache<Pubkey, TokenProgram>,
}

impl Solana {
    /// Build the adapter from the RPC client and the settlement program id.
    pub fn new(rpc: SolanaRPC, program_id: Pubkey) -> Self {
        Self {
            rpc,
            program_id,
            token_programs: Cache::new(TOKEN_PROGRAM_CACHE_CAPACITY),
        }
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

    /// The token program of each of `mints`, fetching only the mints missing
    /// from the cache. Only resolved programs are cached: a mint that is
    /// missing or invalid is read again next time.
    pub async fn token_programs(
        &self,
        mints: impl IntoIterator<Item = Pubkey>,
    ) -> Result<MintPrograms, Error> {
        let (mut programs, unknown): (HashMap<_, _>, Vec<_>) =
            mints
                .into_iter()
                .partition_map(|mint| match self.token_programs.get(&mint) {
                    Some(program) => Either::Left((mint, Ok(program))),
                    None => Either::Right(mint),
                });
        let accounts = self.accounts_snapshot(unknown.iter().copied()).await?;
        for mint in unknown {
            let program = accounts.mint_token_program(&mint);
            if let Ok(program) = program {
                self.token_programs.insert(mint, program);
            }
            programs.insert(mint, program);
        }
        Ok(MintPrograms(programs))
    }
}

/// The token program of each mint passed to [`Solana::token_programs`], or
/// why the mint has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MintPrograms(HashMap<Pubkey, Result<TokenProgram, InvalidMintReason>>);

impl MintPrograms {
    /// `mint`'s token program. A mint that was not looked up reads as not
    /// found.
    pub fn get(&self, mint: Pubkey) -> Result<TokenProgram, InvalidMintReason> {
        self.0
            .get(&mint)
            .copied()
            .unwrap_or(Err(InvalidMintReason::AccountNotFound))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest},
        solana_testlib::{mint_account_json, multiple_accounts_json},
    };

    /// The mock answers the first fetch only, so the second lookup finds the
    /// mint only if the first one cached it.
    #[tokio::test]
    async fn resolved_token_programs_are_cached() {
        let (mint, absent) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([mint_account_json(), serde_json::Value::Null]),
        )]);
        let solana = Solana::new(SolanaRPC::new_mock_with_mocks(mocks), Pubkey::new_unique());

        let expected = MintPrograms(HashMap::from([
            (mint, Ok(TokenProgram::SplToken)),
            (absent, Err(InvalidMintReason::AccountNotFound)),
        ]));
        assert_eq!(
            solana.token_programs([mint, absent]).await.unwrap(),
            expected
        );
        assert_eq!(
            solana.token_programs([mint, absent]).await.unwrap(),
            expected
        );
        assert_eq!(
            MintPrograms::default().get(mint),
            Err(InvalidMintReason::AccountNotFound)
        );
    }
}
