//! The Solana blockchain adapter.
//!
//! Owns the RPC client, the settlement program id and the token program of
//! each mint looked up so far. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

mod accounts;
mod token;

use {
    crate::domain::settlement::find_state_pda,
    cow_settlement_interface::token_program::TokenProgram,
    cow_solana_rpc::{Error, LatestBlockhash, RpcPrioritizationFee, SolanaRPC},
    itertools::{Either, Itertools},
    moka::sync::Cache,
    solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction},
    std::collections::HashMap,
};
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

/// How many mints the token program cache holds.
const TOKEN_PROGRAM_CACHE_CAPACITY: u64 = 10_000;

/// The Solana blockchain adapter.
pub struct Solana {
    rpc: SolanaRPC,
    /// Serves `simulateBundle`, which `rpc` need not support.
    /// TODO: temporary; collapse into `rpc` once a single endpoint serves
    /// every method, see `config::Rpc::bundle_endpoint`.
    bundle_rpc: SolanaRPC,
    program_id: Pubkey,
    /// The token program of each mint looked up so far. Entries never expire:
    /// a mint's owner changes only if the mint is closed and re-created.
    token_programs: Cache<Pubkey, TokenProgram>,
}

impl Solana {
    pub fn new(rpc: SolanaRPC, bundle_rpc: SolanaRPC, program_id: Pubkey) -> Self {
        Self {
            rpc,
            bundle_rpc,
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

    /// The prioritization fees recently paid by transactions locking
    /// `addresses` as writable, one entry per recent slot.
    pub async fn recent_prioritization_fees(
        &self,
        addresses: &[Pubkey],
    ) -> Result<Vec<RpcPrioritizationFee>, Error> {
        self.rpc.recent_prioritization_fees(addresses).await
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
        self.bundle_rpc.simulate_bundle(transactions).await
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

    /// Confirm the settlement program is initialized: the state PDA derived
    /// under the configured id exists and belongs to the program. The PDA is
    /// absent when the id names no deployment, or one running a program
    /// version whose seed differs from the interface crate's.
    pub async fn check_settlement_program(&self) -> Result<(), SettlementProgramError> {
        let state_pda = find_state_pda(&self.program_id).0;
        let accounts = self.rpc.multiple_accounts([state_pda]).await?;
        let initialized = accounts
            .get(&state_pda)
            .is_some_and(|account| account.owner == self.program_id);
        if !initialized {
            return Err(SettlementProgramError::Uninitialized {
                program_id: self.program_id,
                state_pda,
            });
        }
        Ok(())
    }

    /// The token program of each of `mints`, or why the mint has none,
    /// fetching only the mints missing from the cache. Only resolved programs
    /// are cached: a mint that is missing or invalid is read again next time.
    pub async fn token_programs(
        &self,
        mints: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Result<TokenProgram, InvalidMintReason>>, Error> {
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
        Ok(programs)
    }
}

/// Why the settlement program check failed.
#[derive(Debug, thiserror::Error)]
pub enum SettlementProgramError {
    #[error(transparent)]
    Rpc(#[from] Error),
    #[error(
        "settlement program {program_id} has no state PDA at {state_pda}: the configured id or \
         the deployed program version disagrees with the interface crate"
    )]
    Uninitialized {
        program_id: Pubkey,
        state_pda: Pubkey,
    },
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest},
        solana_sdk::account::Account,
        solana_testlib::{account_json, mint_account_json, multiple_accounts_json},
    };

    /// The check passes only with the state PDA owned by the program.
    #[tokio::test]
    async fn settlement_program_check_needs_its_state_pda() {
        let program_id = Pubkey::new_unique();
        let owned = Account {
            owner: program_id,
            ..Default::default()
        };
        let foreign = Account {
            owner: Pubkey::new_unique(),
            ..Default::default()
        };
        for (answer, ok) in [
            (account_json(&owned), true),
            (account_json(&foreign), false),
            (serde_json::Value::Null, false),
        ] {
            let mocks = Mocks::from([(
                RpcRequest::GetMultipleAccounts,
                multiple_accounts_json([answer]),
            )]);
            let solana = Solana::new(
                SolanaRPC::new_mock_with_mocks(mocks.clone()),
                SolanaRPC::new_mock_with_mocks(mocks),
                program_id,
            );
            assert_eq!(solana.check_settlement_program().await.is_ok(), ok);
        }
    }

    /// The mock answers the first fetch only, so the second lookup finds the
    /// mint only if the first one cached it.
    #[tokio::test]
    async fn resolved_token_programs_are_cached() {
        let (mint, absent) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([mint_account_json(), serde_json::Value::Null]),
        )]);
        let solana = Solana::new(
            SolanaRPC::new_mock_with_mocks(mocks.clone()),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let expected = HashMap::from([
            (mint, Ok(TokenProgram::SplToken)),
            (absent, Err(InvalidMintReason::AccountNotFound)),
        ]);
        assert_eq!(
            solana.token_programs([mint, absent]).await.unwrap(),
            expected
        );
        assert_eq!(
            solana.token_programs([mint, absent]).await.unwrap(),
            expected
        );
    }
}
