//! Countersigning of pending sponsored creation transactions.

use {
    crate::infra::{db, order_events},
    anyhow::{Context, Result, ensure},
    chain_types::solana::IntentHash,
    cow_solana_rpc::SolanaRPC,
    cow_solana_signer::Signer,
    database::solana::OrderEventLabel,
    solana_sdk::{account::Account, pubkey::Pubkey, transaction::VersionedTransaction},
    sqlx::PgPool,
    std::collections::HashMap,
    tokio::sync::Mutex,
};

/// Holds the funder signer and countersigns the stored creation transactions
/// of pending sponsored orders.
pub struct Sponsor {
    signer: Signer,
    rpc: SolanaRPC,
    pool: PgPool,
    max_displaced_creations: usize,
    /// Held while a displaced-creation run sends. Runs are detached, so a
    /// slow one would overlap the next cycles, resend the creations still
    /// pending, and let more than `max_displaced_creations` go out at once. A
    /// cycle that finds a run in flight skips.
    displacing: Mutex<()>,
}

/// A stored sponsored creation the indexer has not seen on chain.
struct PendingCreation {
    uid: IntentHash,
    /// The order account the creation opens.
    order_pda: Pubkey,
    /// The owner-signed transaction as stored, countersigned at send time.
    bytes: Vec<u8>,
    transaction: VersionedTransaction,
}

/// The stored creation's blockhash is past its last valid height, so the
/// creation can never land.
#[derive(Debug, thiserror::Error)]
#[error("the creation blockhash expired")]
struct BlockhashExpired;

impl Sponsor {
    pub fn new(
        signer: Signer,
        rpc: SolanaRPC,
        pool: PgPool,
        max_displaced_creations: usize,
    ) -> Self {
        Self {
            signer,
            rpc,
            pool,
            max_displaced_creations,
            displacing: Mutex::new(()),
        }
    }

    /// The fully signed creation transactions the given orders still need on
    /// chain. Orders already created contribute nothing. An error means one
    /// creation can no longer land (dead blockhash) or did not countersign,
    /// so the solution containing it cannot settle. A dead creation is also
    /// expired in the database, which drops its order from the next cut.
    pub async fn countersign_creations(
        &self,
        uids: impl Iterator<Item = IntentHash>,
    ) -> Result<Vec<Vec<u8>>> {
        let uids: Vec<Vec<u8>> = uids.map(|uid| uid.0.to_vec()).collect();
        let pending = db::pending_creations(&self.pool, &uids).await?;
        let mut creations = Vec::with_capacity(pending.len());
        for stored in pending {
            let creation = match self.countersign(&stored.transaction).await {
                Err(err) if err.is::<BlockhashExpired>() => {
                    self.expire(&stored.uid.0).await;
                    Err(err)
                }
                creation => creation,
            }
            .with_context(|| format!("order 0x{}", const_hex::encode(stored.uid.0)))?;
            creations.push(creation);
        }
        Ok(creations)
    }

    /// Send the creations of the given pending sponsored orders without a
    /// settlement, at most `max_displaced_creations` of them, so the orders
    /// outlive their creation blockhash. Returns the orders whose creation
    /// went out. Failures only log: the order keeps its stored creation and
    /// the cut drops it once the blockhash dies.
    pub async fn create_displaced(
        &self,
        uids: impl Iterator<Item = IntentHash>,
    ) -> Vec<IntentHash> {
        let uids: Vec<Vec<u8>> = uids.map(|uid| uid.0.to_vec()).collect();
        if uids.is_empty() || self.max_displaced_creations == 0 {
            return Vec::new();
        }
        let pending = match db::pending_creations(&self.pool, &uids).await {
            Ok(pending) => pending,
            Err(err) => {
                tracing::warn!(?err, "failed to read displaced sponsored creations");
                return Vec::new();
            }
        };
        let pending = pending
            .into_iter()
            .filter_map(|stored| match bincode::deserialize(&stored.transaction) {
                Ok(transaction) => Some(PendingCreation {
                    uid: IntentHash(stored.uid.0),
                    order_pda: Pubkey::new_from_array(stored.order_pda.0),
                    bytes: stored.transaction,
                    transaction,
                }),
                Err(err) => {
                    let order_uid = const_hex::encode_prefixed(stored.uid.0);
                    tracing::warn!(%order_uid, ?err, "stored creation does not decode");
                    None
                }
            })
            .collect();
        self.send_displaced(pending).await
    }

    /// Send the creatable ones among the pending creations, in order, up to
    /// the cap. A creation whose order account exists landed earlier and
    /// waits for the indexer, so it is not sent again. One that opens an
    /// account with the funder's lamports is skipped, since that rent goes
    /// to the account's owner. A run still in flight makes the call skip,
    /// the next cycle picks up what is still pending.
    async fn send_displaced(&self, pending: Vec<PendingCreation>) -> Vec<IntentHash> {
        let Ok(_displacing) = self.displacing.try_lock() else {
            tracing::debug!("displaced creations still going out, skipping the cycle");
            return Vec::new();
        };
        let funder = self.signer.pubkey();
        let existing = match self
            .rpc
            .multiple_accounts(pending.iter().flat_map(|creation| {
                funder_paid_accounts(&creation.transaction, &funder)
                    .into_iter()
                    .chain([creation.order_pda])
            }))
            .await
        {
            Ok(existing) => existing,
            Err(err) => {
                tracing::warn!(?err, "displaced creation account lookup failed");
                return Vec::new();
            }
        };
        let creatable = pending
            .iter()
            .filter(|creation| {
                !existing.contains_key(&creation.order_pda)
                    && !opens_funder_paid_account(&creation.transaction, &funder, &existing)
            })
            .take(self.max_displaced_creations);
        let mut sent = Vec::new();
        for creation in creatable {
            let order_uid = const_hex::encode_prefixed(creation.uid.0);
            let signed = match self.countersign(&creation.bytes).await {
                Ok(signed) => signed,
                Err(err) if err.is::<BlockhashExpired>() => {
                    self.expire(&creation.uid.0).await;
                    continue;
                }
                Err(err) => {
                    tracing::warn!(%order_uid, ?err, "failed to countersign a displaced creation");
                    continue;
                }
            };
            let submitted = async {
                let transaction: VersionedTransaction = bincode::deserialize(&signed)?;
                anyhow::Ok(self.rpc.send_transaction(&transaction).await?)
            };
            match submitted.await {
                Ok(signature) => {
                    tracing::info!(%order_uid, %signature, "sent a displaced order's creation");
                    sent.push(creation.uid);
                }
                Err(err) => {
                    tracing::warn!(%order_uid, ?err, "failed to send a displaced creation")
                }
            }
        }
        sent
    }

    /// Lower a dead creation's stored deadline below the chain height, which
    /// drops the order from the next cut, and record the order as invalid,
    /// since the cut drops it without an event. On failure the stored upper
    /// bound still drops it later.
    async fn expire(&self, uid: &[u8]) {
        let order_uid = const_hex::encode_prefixed(uid);
        let expired = async {
            let height = u64::from(self.rpc.block_height().await?);
            db::expire_creation(&self.pool, uid, i64::try_from(height)?.saturating_sub(1)).await?;
            order_events::store(
                &self.pool,
                [IntentHash(uid.try_into()?)],
                OrderEventLabel::Invalid,
            )
            .await
        };
        match expired.await {
            Ok(()) => tracing::info!(%order_uid, "sponsored creation expired, dropping the order"),
            Err(err) => {
                tracing::warn!(%order_uid, ?err, "failed to expire a dead sponsored creation")
            }
        }
    }

    /// Fill the funder's fee payer slot of one stored creation transaction.
    /// Placement pinned the funder as fee payer and first key, so the
    /// signature belongs in the first slot.
    async fn countersign(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut transaction: VersionedTransaction =
            bincode::deserialize(bytes).context("stored creation does not decode")?;
        ensure!(
            transaction.message.static_account_keys().first() == Some(&self.signer.pubkey()),
            "stored creation does not name the funder as fee payer"
        );
        let valid = self
            .rpc
            .is_blockhash_valid(transaction.message.recent_blockhash())
            .await
            .context("blockhash validity check failed")?;
        ensure!(valid, BlockhashExpired);
        let signature = self
            .signer
            .sign_message(&transaction.message.serialize())
            .await
            .context("failed to countersign the creation")?;
        match transaction.signatures.first_mut() {
            Some(slot) => *slot = signature,
            None => anyhow::bail!("stored creation carries no signature slots"),
        }
        bincode::serialize(&transaction).context("serialize countersigned creation")
    }
}

/// The associated token accounts a creation opens with the funder as payer.
/// Creations carry no lookup tables, so the static keys resolve every index.
fn funder_paid_accounts(creation: &VersionedTransaction, funder: &Pubkey) -> Vec<Pubkey> {
    let keys = creation.message.static_account_keys();
    creation
        .message
        .instructions()
        .iter()
        .filter(|instruction| {
            keys.get(usize::from(instruction.program_id_index))
                == Some(&spl_associated_token_account_interface::program::ID)
        })
        .filter_map(|instruction| {
            let payer = keys.get(usize::from(*instruction.accounts.first()?))?;
            let account = keys.get(usize::from(*instruction.accounts.get(1)?))?;
            (payer == funder).then_some(*account)
        })
        .collect()
}

/// Whether the creation opens a new account with the funder's lamports. An
/// account already in `existing` costs nothing to create idempotently.
fn opens_funder_paid_account(
    creation: &VersionedTransaction,
    funder: &Pubkey,
    existing: &HashMap<Pubkey, Account>,
) -> bool {
    funder_paid_accounts(creation, funder)
        .iter()
        .any(|account| !existing.contains_key(account))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, MocksMap, RpcRequest},
        solana_sdk::{
            hash::Hash,
            message::Message,
            pubkey::Pubkey,
            signature::Signature,
            signer::{Signer as _, keypair::Keypair},
        },
    };

    /// A pending creation the funder pays for that opens `order_pda`.
    fn pending_creation(funder: &Keypair, order_pda: Pubkey) -> PendingCreation {
        let message = Message::new_with_blockhash(
            &[solana_sdk::instruction::Instruction::new_with_bytes(
                Pubkey::new_unique(),
                &[],
                vec![
                    solana_sdk::instruction::AccountMeta::new(funder.pubkey(), true),
                    solana_sdk::instruction::AccountMeta::new(order_pda, false),
                ],
            )],
            Some(&funder.pubkey()),
            &Hash::new_unique(),
        );
        let transaction = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        };
        PendingCreation {
            uid: IntentHash(order_pda.to_bytes()),
            order_pda,
            bytes: bincode::serialize(&transaction).unwrap(),
            transaction,
        }
    }

    /// Countersigning fills exactly the funder's slot and leaves the owner's
    /// signature intact and valid.
    #[tokio::test]
    async fn countersign_fills_the_fee_payer_slot() {
        let funder = Keypair::new();
        let owner = Keypair::new();
        let instruction = solana_sdk::instruction::Instruction::new_with_bytes(
            Pubkey::new_unique(),
            &[],
            vec![
                solana_sdk::instruction::AccountMeta::new(funder.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new(owner.pubkey(), true),
            ],
        );
        let message = Message::new_with_blockhash(
            &[instruction],
            Some(&funder.pubkey()),
            &Hash::new_unique(),
        );
        let serialized = message.serialize();
        let mut signatures = vec![Signature::default(); 2];
        signatures[1] = owner.sign_message(&serialized);
        let transaction = VersionedTransaction {
            signatures,
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        };

        let sponsor = Sponsor::new(
            Signer::Keypair(funder.insecure_clone()),
            SolanaRPC::new_mock_with_mocks(Mocks::from([(
                RpcRequest::IsBlockhashValid,
                serde_json::json!({
                    "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                    "value": true,
                }),
            )])),
            PgPool::connect_lazy("postgresql://").unwrap(),
            0,
        );
        let signed = sponsor
            .countersign(&bincode::serialize(&transaction).unwrap())
            .await
            .unwrap();
        let signed: VersionedTransaction = bincode::deserialize(&signed).unwrap();
        assert!(signed.signatures[0].verify(funder.pubkey().as_ref(), &serialized));
        assert!(signed.signatures[1].verify(owner.pubkey().as_ref(), &serialized));
    }

    /// Only an account the funder pays for and the chain does not hold yet
    /// blocks a displaced creation.
    #[test]
    fn missing_funder_paid_accounts_block_a_displaced_creation() {
        let funder = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let funder_paid = Pubkey::new_unique();
        let create = |payer: Pubkey, account: Pubkey| {
            solana_sdk::instruction::Instruction::new_with_bytes(
                spl_associated_token_account_interface::program::ID,
                &[1],
                vec![
                    solana_sdk::instruction::AccountMeta::new(payer, true),
                    solana_sdk::instruction::AccountMeta::new(account, false),
                    solana_sdk::instruction::AccountMeta::new_readonly(owner, false),
                    solana_sdk::instruction::AccountMeta::new_readonly(Pubkey::new_unique(), false),
                    solana_sdk::instruction::AccountMeta::new_readonly(Pubkey::new_unique(), false),
                    solana_sdk::instruction::AccountMeta::new_readonly(Pubkey::new_unique(), false),
                ],
            )
        };
        let message = Message::new_with_blockhash(
            &[
                create(funder, funder_paid),
                create(owner, Pubkey::new_unique()),
            ],
            Some(&funder),
            &Hash::new_unique(),
        );
        let creation = VersionedTransaction {
            signatures: vec![Signature::default(); 2],
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        };

        assert_eq!(funder_paid_accounts(&creation, &funder), vec![funder_paid]);
        assert!(opens_funder_paid_account(
            &creation,
            &funder,
            &HashMap::new()
        ));
        let existing = HashMap::from([(funder_paid, Account::default())]);
        assert!(!opens_funder_paid_account(&creation, &funder, &existing));
    }

    /// A creation whose order account already exists landed earlier and
    /// waits for the indexer, so it is not sent again.
    #[tokio::test]
    async fn landed_creations_are_not_resent() {
        let funder = Keypair::new();
        let landed = pending_creation(&funder, Pubkey::new_unique());
        let fresh = pending_creation(&funder, Pubkey::new_unique());
        // The lookup finds the landed order's account and not the fresh
        // one's. Every blockhash check passes, and the mock answers a send
        // with the sent transaction's own signature.
        let valid = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": true,
        });
        let mocks: MocksMap = [
            (
                RpcRequest::GetMultipleAccounts,
                serde_json::json!({
                    "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                    "value": [solana_testlib::account_json(&Account::default()), null],
                }),
            ),
            (RpcRequest::IsBlockhashValid, valid.clone()),
            (RpcRequest::IsBlockhashValid, valid),
        ]
        .into_iter()
        .collect();
        let sponsor = Sponsor::new(
            Signer::Keypair(funder.insecure_clone()),
            SolanaRPC::new_mock_with_mocks_map(mocks),
            PgPool::connect_lazy("postgres://localhost/unused").unwrap(),
            10,
        );

        let fresh_uid = fresh.uid;
        let sent = sponsor.send_displaced(vec![landed, fresh]).await;
        assert_eq!(sent, vec![fresh_uid]);
    }

    /// A cycle that finds a run still in flight skips instead of signing the
    /// same creations again.
    #[tokio::test]
    async fn a_run_in_flight_makes_the_next_cycle_skip() {
        let funder = Keypair::new();
        let fresh = pending_creation(&funder, Pubkey::new_unique());
        // Nothing but the run in flight stops the send.
        let mocks: MocksMap = [
            (
                RpcRequest::GetMultipleAccounts,
                serde_json::json!({
                    "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                    "value": [null],
                }),
            ),
            (
                RpcRequest::IsBlockhashValid,
                serde_json::json!({
                    "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                    "value": true,
                }),
            ),
        ]
        .into_iter()
        .collect();
        let sponsor = Sponsor::new(
            Signer::Keypair(funder.insecure_clone()),
            SolanaRPC::new_mock_with_mocks_map(mocks),
            PgPool::connect_lazy("postgres://localhost/unused").unwrap(),
            10,
        );

        let _in_flight = sponsor.displacing.lock().await;
        assert!(sponsor.send_displaced(vec![fresh]).await.is_empty());
    }

    /// A dead blockhash refuses the countersign.
    #[tokio::test]
    async fn countersign_refuses_a_dead_blockhash() {
        let funder = Keypair::new();
        let message = Message::new_with_blockhash(
            &[solana_sdk::instruction::Instruction::new_with_bytes(
                Pubkey::new_unique(),
                &[],
                vec![solana_sdk::instruction::AccountMeta::new(
                    funder.pubkey(),
                    true,
                )],
            )],
            Some(&funder.pubkey()),
            &Hash::new_unique(),
        );
        let transaction = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        };
        let sponsor = Sponsor::new(
            Signer::Keypair(funder.insecure_clone()),
            SolanaRPC::new_mock_with_mocks(Mocks::from([(
                RpcRequest::IsBlockhashValid,
                serde_json::json!({
                    "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                    "value": false,
                }),
            )])),
            PgPool::connect_lazy("postgresql://").unwrap(),
            0,
        );
        let result = sponsor
            .countersign(&bincode::serialize(&transaction).unwrap())
            .await;
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    /// A dead blockhash pulls the order's stored creation deadline below the
    /// chain height, never above what was stored, and records the order as
    /// invalid.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_a_dead_blockhash_expires_the_creation() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;
        let funder = Keypair::new();
        let message = Message::new_with_blockhash(
            &[solana_sdk::instruction::Instruction::new_with_bytes(
                Pubkey::new_unique(),
                &[],
                vec![solana_sdk::instruction::AccountMeta::new(
                    funder.pubkey(),
                    true,
                )],
            )],
            Some(&funder.pubkey()),
            &Hash::new_unique(),
        );
        let creation = bincode::serialize(&VersionedTransaction {
            signatures: vec![Signature::default()],
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        })
        .unwrap();
        for (n, stored, expected) in [(1u8, 5_000i64, 999i64), (2, 500, 500)] {
            sqlx::query(
                r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind, partially_fillable,
    app_data, creation_timestamp, order_pda, presigned_transaction, last_valid_block_height)
VALUES ($1, $2, $2, $2, $2, $2, 1000, 2000, 2000, 'sell'::solana.OrderKind, false, $2,
    now(), $3, $4, $5)
                "#,
            )
            .bind(vec![n; 32])
            .bind(vec![0xAAu8; 32])
            .bind(vec![n | 0x80; 32])
            .bind(creation.clone())
            .bind(stored)
            .execute(&pool)
            .await
            .unwrap();
            let sponsor = Sponsor::new(
                Signer::Keypair(funder.insecure_clone()),
                SolanaRPC::new_mock_with_mocks(Mocks::from([
                    (
                        RpcRequest::IsBlockhashValid,
                        serde_json::json!({
                            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                            "value": false,
                        }),
                    ),
                    (RpcRequest::GetBlockHeight, serde_json::json!(1_000u64)),
                ])),
                pool.clone(),
                0,
            );
            let err = sponsor
                .countersign_creations(std::iter::once(IntentHash([n; 32])))
                .await
                .unwrap_err();
            assert!(err.is::<BlockhashExpired>());
            let height: i64 = sqlx::query_scalar(
                "SELECT last_valid_block_height FROM solana.orders WHERE uid = $1",
            )
            .bind(vec![n; 32])
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(height, expected);
            let labels: Vec<String> = sqlx::query_scalar(
                "SELECT label::text FROM solana.order_events WHERE order_uid = $1",
            )
            .bind(vec![n; 32])
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(labels, ["invalid"]);
        }
    }
}
