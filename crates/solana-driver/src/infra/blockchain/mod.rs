//! The Solana blockchain adapter.
//!
//! Owns the RPC client, the settlement program id and the facts of each
//! mint looked up so far. Mirrors the EVM driver's `infra/blockchain/mod.rs`
//! (`struct Ethereum`).

mod accounts;
mod token;

use {
    crate::domain::Slot,
    cow_settlement_interface::token_program::TokenProgram,
    cow_solana_rpc::{
        Error,
        LatestBlockhash,
        RpcPrioritizationFee,
        RpcSimulateBundleTransactionResult,
        SolanaRPC,
        UiTransactionError,
    },
    itertools::{Either, Itertools},
    moka::sync::Cache,
    solana_sdk::{
        clock::MAX_PROCESSING_AGE,
        hash::Hash,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::{collections::HashMap, time::Duration},
};
pub use {
    accounts::{AccountsSnapshot, InvalidMintReason, MintInfo, TokenAccountState},
    token::{
        associated_token_address,
        close_token_account,
        create_associated_token_account_idempotent,
        require_token_balance,
    },
};

/// How many mints the cache holds.
const MINT_CACHE_CAPACITY: u64 = 10_000;

/// How often a wait on a slot reads the node's current slot. Shorter than a
/// slot, so the wait ends within a slot of its target.
const SLOT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How often a transaction waiting for confirmation goes out again, about two
/// mainnet slots, so a dropped copy delays the landing by at most that.
const RESEND_INTERVAL: Duration = Duration::from_millis(400);

/// A simulated transaction's outcome and the slot it ran against.
pub struct TransactionSimulation {
    pub slot: Slot,
    pub err: Option<UiTransactionError>,
    pub logs: Option<Vec<String>>,
}

/// A simulated bundle's per-transaction results, in order and ending at the
/// first failure, and the slot it ran against.
pub struct BundleSimulation {
    pub slot: Slot,
    pub results: Vec<RpcSimulateBundleTransactionResult>,
}

/// The Solana blockchain adapter.
pub struct Solana {
    rpc: SolanaRPC,
    /// Serves `simulateBundle`, which `rpc` need not support.
    /// TODO: temporary; collapse into `rpc` once a single endpoint serves
    /// every method, see `config::Rpc::bundle_endpoint`.
    bundle_rpc: SolanaRPC,
    program_id: Pubkey,
    /// The facts of each mint looked up so far. Entries never expire: a
    /// mint's owner and extensions change only if the mint is closed and
    /// re-created.
    mints: Cache<Pubkey, MintInfo>,
}

impl Solana {
    pub fn new(rpc: SolanaRPC, bundle_rpc: SolanaRPC, program_id: Pubkey) -> Self {
        Self {
            rpc,
            bundle_rpc,
            program_id,
            mints: Cache::new(MINT_CACHE_CAPACITY),
        }
    }

    /// The settlement program id this driver settles against.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// A confirmed blockhash that expires at about `deadline_slot`, so a
    /// transaction signed with it cannot land much past the deadline.
    ///
    /// A blockhash stays usable for `MAX_PROCESSING_AGE` blocks above its
    /// own, so this is the hash of the block that many blocks below the
    /// height the chain reaches at the deadline if no slot until then is
    /// skipped. Each skipped slot moves the expiry one slot past the
    /// deadline. A deadline at least one blockhash lifetime out, or a failed
    /// search, yields the latest blockhash.
    pub async fn blockhash_expiring_at(&self, deadline_slot: u64) -> Result<Hash, Error> {
        let latest = self.rpc.latest_confirmed_blockhash().await?;
        let lifetime = MAX_PROCESSING_AGE as u64;
        let remaining = deadline_slot.saturating_sub(latest.slot);
        if remaining >= lifetime {
            return Ok(latest.blockhash);
        }
        match self.blockhash_below(&latest, lifetime - remaining).await {
            Ok(Some(blockhash)) => Ok(blockhash),
            found => {
                tracing::warn!(
                    ?found,
                    deadline_slot,
                    "no blockhash expiring at the deadline, signing with the latest"
                );
                Ok(latest.blockhash)
            }
        }
    }

    /// The hash of the block `age` blocks below the latest one, `None` when
    /// the search finds no block at that height. Blocks sit at distinct
    /// slots, so the `2 * age` slots below the latest hold it unless half of
    /// them are skipped.
    async fn blockhash_below(
        &self,
        latest: &LatestBlockhash,
        age: u64,
    ) -> Result<Option<Hash>, Error> {
        let slots = self
            .rpc
            .blocks(latest.slot.saturating_sub(2 * age), latest.slot)
            .await?;
        let Some(slot) = block_below(&slots, latest.slot, age) else {
            return Ok(None);
        };
        let (blockhash, height) = self.rpc.block(slot).await?;
        let expiry = u64::from(height) + age + MAX_PROCESSING_AGE as u64;
        Ok((expiry == u64::from(latest.last_valid_block_height)).then_some(blockhash))
    }

    /// The node's current slot at the client's commitment level.
    pub async fn slot(&self) -> Result<u64, Error> {
        self.rpc.slot().await
    }

    /// Run `work` until the node reaches `slot`, `None` if the slot comes
    /// first. `work` starts even when the slot has passed, and a failed slot
    /// read counts as the slot not reached.
    pub async fn until_slot<T>(&self, slot: u64, work: impl Future<Output = T>) -> Option<T> {
        let reached = async {
            while !self.slot().await.is_ok_and(|current| current >= slot) {
                tokio::time::sleep(SLOT_POLL_INTERVAL).await;
            }
        };
        tokio::select! {
            biased;
            done = work => Some(done),
            () = reached => None,
        }
    }

    /// The prioritization fees recently paid by transactions locking
    /// `addresses` as writable, one entry per recent slot.
    pub async fn recent_prioritization_fees(
        &self,
        addresses: &[Pubkey],
    ) -> Result<Vec<RpcPrioritizationFee>, Error> {
        self.rpc.recent_prioritization_fees(addresses).await
    }

    /// Simulate a signed transaction without sending it.
    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<TransactionSimulation, Error> {
        let response = self.rpc.simulate_transaction(transaction).await?;
        Ok(TransactionSimulation {
            slot: Slot(response.context.slot),
            err: response.value.err,
            logs: response.value.logs,
        })
    }

    /// See [`SolanaRPC::simulate_bundle`].
    pub async fn simulate_bundle(
        &self,
        transactions: &[VersionedTransaction],
    ) -> Result<BundleSimulation, Error> {
        let response = self.bundle_rpc.simulate_bundle(transactions, &[]).await?;
        Ok(BundleSimulation {
            slot: Slot(response.context.slot),
            results: response.value,
        })
    }

    /// Send a signed transaction and wait for confirmation, sending the same
    /// bytes again every [`RESEND_INTERVAL`] until it lands. The copies skip
    /// the preflight, so they go out only after the first send passed it. The
    /// transaction cannot land twice: the cluster processes a transaction at
    /// most once, and only while its blockhash is valid. The wait has no end of
    /// its own, callers bound it.
    ///
    /// An `ErrorKind::TransactionError` means the transaction landed and
    /// failed, so it cannot land again. A rejected preflight is an
    /// `ErrorKind::RpcError` instead.
    pub async fn send_and_confirm_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        let signature = self.rpc.send_transaction_once(transaction).await?;
        loop {
            tokio::time::sleep(RESEND_INTERVAL).await;
            if let Some(outcome) = self.rpc.signature_status(&signature).await? {
                return outcome.map(|()| signature).map_err(Error::from);
            }
            // The status decides the outcome, so a failed copy is only logged.
            if let Err(err) = self.rpc.resend_transaction(transaction).await {
                tracing::debug!(?err, "transaction resend failed");
            }
        }
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

    /// The facts of each of `mints`, or why the mint has none, fetching only
    /// the mints missing from the cache. Only resolved mints are cached: a
    /// mint that is missing or invalid is read again next time.
    pub async fn mints(
        &self,
        mints: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Result<MintInfo, InvalidMintReason>>, Error> {
        let (mut infos, unknown): (HashMap<_, _>, Vec<_>) =
            mints
                .into_iter()
                .partition_map(|mint| match self.mints.get(&mint) {
                    Some(info) => Either::Left((mint, Ok(info))),
                    None => Either::Right(mint),
                });
        let accounts = self.accounts_snapshot(unknown.iter().copied()).await?;
        for mint in unknown {
            let info = accounts.mint_info(&mint);
            if let Ok(info) = info {
                self.mints.insert(mint, info);
            }
            infos.insert(mint, info);
        }
        Ok(infos)
    }

    /// The token program of each of `mints`, see [`Self::mints`].
    pub async fn token_programs(
        &self,
        mints: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Result<TokenProgram, InvalidMintReason>>, Error> {
        Ok(self
            .mints(mints)
            .await?
            .into_iter()
            .map(|(mint, info)| (mint, info.map(|info| info.program)))
            .collect())
    }
}

/// The slot of the block `age` blocks below the one at `latest`, given the
/// ascending slots of the blocks up to `latest`.
fn block_below(slots: &[u64], latest: u64, age: u64) -> Option<u64> {
    if slots.last() != Some(&latest) {
        return None;
    }
    slots.iter().rev().nth(usize::try_from(age).ok()?).copied()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, MocksMap, RpcRequest},
        serde_json::json,
        solana_sdk::transaction::TransactionError,
        solana_testlib::{mint_account_json, multiple_accounts_json},
    };

    /// The latest block sits at slot 1000 and height 900, and slot 990 is
    /// skipped. A deadline 25 slots out takes the block 125 blocks below,
    /// at height 775 and slot 874, whose blockhash expires at height 925.
    #[tokio::test]
    async fn the_settlement_blockhash_expires_at_the_deadline() {
        let (latest, below) = (Hash::new_unique(), Hash::new_unique());
        let slots: Vec<u64> = (750..=1000).filter(|slot| *slot != 990).collect();
        let solana = |block_height: u64| {
            let mocks = Mocks::from([
                (
                    RpcRequest::GetLatestBlockhash,
                    json!({
                        "context": {"slot": 1000},
                        "value": {"blockhash": latest.to_string(), "lastValidBlockHeight": 1050},
                    }),
                ),
                (RpcRequest::GetBlocks, json!(slots)),
                (
                    RpcRequest::GetBlock,
                    json!({
                        "previousBlockhash": Hash::default().to_string(),
                        "blockhash": below.to_string(),
                        "parentSlot": 873,
                        "blockHeight": block_height,
                    }),
                ),
            ]);
            Solana::new(
                SolanaRPC::new_mock_with_mocks(mocks.clone()),
                SolanaRPC::new_mock_with_mocks(mocks),
                Pubkey::new_unique(),
            )
        };

        assert_eq!(block_below(&slots, 1000, 125), Some(874));
        assert_eq!(block_below(&slots, 999, 125), None);
        assert_eq!(
            solana(775).blockhash_expiring_at(1025).await.unwrap(),
            below
        );
        // A block whose height disagrees with the latest one's.
        assert_eq!(
            solana(776).blockhash_expiring_at(1025).await.unwrap(),
            latest
        );
        // A deadline a lifetime out is the latest blockhash's own expiry.
        assert_eq!(
            solana(775).blockhash_expiring_at(1150).await.unwrap(),
            latest
        );
    }

    /// A failed slot read keeps the wait going, the node reaching the slot
    /// ends it, and work that finishes first wins.
    #[tokio::test]
    async fn a_wait_ends_at_its_slot() {
        let slots = MocksMap::from_iter([
            (RpcRequest::GetSlot, json!(99)),
            (RpcRequest::GetSlot, json!(100)),
        ]);
        let solana = Solana::new(
            SolanaRPC::new_mock_with_failures(
                slots.clone(),
                [(RpcRequest::GetSlot, TransactionError::AccountInUse)],
            ),
            SolanaRPC::new_mock_with_mocks_map(slots),
            Pubkey::new_unique(),
        );
        assert_eq!(
            solana.until_slot(100, std::future::pending::<()>()).await,
            None
        );
        // The wait read every queued slot, so the next read gets the mock's
        // unqueued answer.
        assert_eq!(solana.slot().await.unwrap(), 0);
        assert_eq!(solana.until_slot(100, async { 7 }).await, Some(7));
    }

    /// The cluster confirms the transaction at the third status check, 1.2s
    /// in. The copies sent at 400ms and 800ms take the second and third queued
    /// send answers, so the next send gets the mismatched fourth and fails.
    #[tokio::test(start_paused = true)]
    async fn an_unconfirmed_transaction_goes_out_again() {
        let transaction = VersionedTransaction {
            signatures: vec![Signature::from([1; 64])],
            message: Default::default(),
        };
        let sent = json!(transaction.signatures[0].to_string());
        let pending = json!({ "context": { "slot": 1 }, "value": [null] });
        let mocks = MocksMap::from_iter([
            (RpcRequest::SendTransaction, sent.clone()),
            (RpcRequest::SendTransaction, sent.clone()),
            (RpcRequest::SendTransaction, sent),
            (
                RpcRequest::SendTransaction,
                json!(Signature::from([2; 64]).to_string()),
            ),
            (RpcRequest::GetSignatureStatuses, pending.clone()),
            (RpcRequest::GetSignatureStatuses, pending),
            (
                RpcRequest::GetSignatureStatuses,
                json!({
                    "context": { "slot": 1 },
                    "value": [{
                        "slot": 1,
                        "confirmations": null,
                        "status": { "Ok": null },
                        "err": null,
                        "confirmationStatus": "confirmed",
                    }],
                }),
            ),
        ]);
        let solana = Solana::new(
            SolanaRPC::new_mock_with_mocks_map(mocks.clone()),
            SolanaRPC::new_mock_with_mocks_map(mocks),
            Pubkey::new_unique(),
        );
        assert_eq!(
            solana
                .send_and_confirm_transaction(&transaction)
                .await
                .unwrap(),
            transaction.signatures[0]
        );
        assert!(
            solana
                .send_and_confirm_transaction(&transaction)
                .await
                .is_err()
        );
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
