//! Countersigning of sponsored creation transactions for winning solutions.

use {
    crate::infra::{db, order_events},
    anyhow::{Context, Result, ensure},
    chain_types::solana::IntentHash,
    cow_solana_rpc::SolanaRPC,
    cow_solana_signer::Signer,
    database::solana::OrderEventLabel,
    solana_sdk::transaction::VersionedTransaction,
    sqlx::PgPool,
};

/// Holds the funder signer and countersigns the stored creation transactions
/// of winning sponsored orders right before their settlement is dispatched.
pub struct Sponsor {
    signer: Signer,
    rpc: SolanaRPC,
    pool: PgPool,
}

/// The stored creation's blockhash is past its last valid height, so the
/// creation can never land.
#[derive(Debug, thiserror::Error)]
#[error("the creation blockhash expired")]
struct BlockhashExpired;

impl Sponsor {
    pub fn new(signer: Signer, rpc: SolanaRPC, pool: PgPool) -> Self {
        Self { signer, rpc, pool }
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
        for (uid, bytes) in pending {
            let creation = match self.countersign(&bytes).await {
                Err(err) if err.is::<BlockhashExpired>() => {
                    self.expire(&uid).await;
                    Err(err)
                }
                creation => creation,
            }
            .with_context(|| format!("order 0x{}", const_hex::encode(uid)))?;
            creations.push(creation);
        }
        Ok(creations)
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest},
        solana_sdk::{
            hash::Hash,
            message::Message,
            pubkey::Pubkey,
            signature::Signature,
            signer::{Signer as _, keypair::Keypair},
        },
    };

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
        );
        let signed = sponsor
            .countersign(&bincode::serialize(&transaction).unwrap())
            .await
            .unwrap();
        let signed: VersionedTransaction = bincode::deserialize(&signed).unwrap();
        assert!(signed.signatures[0].verify(funder.pubkey().as_ref(), &serialized));
        assert!(signed.signatures[1].verify(owner.pubkey().as_ref(), &serialized));
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
