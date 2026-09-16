//! Countersigning of sponsored creation transactions for winning solutions.

use {
    crate::infra::db,
    anyhow::{Context, Result, ensure},
    chain_types::solana::IntentHash,
    cow_solana_rpc::SolanaRPC,
    solana_sdk::{
        signer::{Signer, keypair::Keypair},
        transaction::VersionedTransaction,
    },
    sqlx::PgPool,
};

/// Holds the funder key and countersigns the stored creation transactions of
/// winning sponsored orders right before their settlement is dispatched.
pub struct Sponsor {
    keypair: Keypair,
    rpc: SolanaRPC,
    pool: PgPool,
}

impl Sponsor {
    pub fn new(keypair: Keypair, rpc: SolanaRPC, pool: PgPool) -> Self {
        Self { keypair, rpc, pool }
    }

    /// The fully signed creation transactions the given orders still need on
    /// chain. Orders already created contribute nothing. An error means one
    /// creation can no longer land (dead blockhash) or did not countersign,
    /// so the solution containing it cannot settle.
    pub async fn creations(&self, uids: impl Iterator<Item = IntentHash>) -> Result<Vec<Vec<u8>>> {
        let uids: Vec<Vec<u8>> = uids.map(|uid| uid.0.to_vec()).collect();
        let pending = db::pending_creations(&self.pool, &uids).await?;
        let mut creations = Vec::with_capacity(pending.len());
        for (uid, bytes) in pending {
            let creation = self
                .countersign(&bytes)
                .await
                .with_context(|| format!("order 0x{}", const_hex::encode(uid)))?;
            creations.push(creation);
        }
        Ok(creations)
    }

    /// Fill the funder's fee payer slot of one stored creation transaction.
    /// Placement pinned the funder as fee payer and first key, so the
    /// signature belongs in the first slot.
    async fn countersign(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut transaction: VersionedTransaction =
            bincode::deserialize(bytes).context("stored creation does not decode")?;
        ensure!(
            transaction.message.static_account_keys().first() == Some(&self.keypair.pubkey()),
            "stored creation does not name the funder as fee payer"
        );
        let valid = self
            .rpc
            .is_blockhash_valid(transaction.message.recent_blockhash())
            .await
            .context("blockhash validity check failed")?;
        ensure!(valid, "the creation blockhash expired");
        let signature = self.keypair.sign_message(&transaction.message.serialize());
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
        solana_sdk::{hash::Hash, message::Message, pubkey::Pubkey, signature::Signature},
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
            funder.insecure_clone(),
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
            funder.insecure_clone(),
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
}
