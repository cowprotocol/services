//! This module is responsible for associating auction ids with transaction hashes.
//!
//! see database/sql/V037__auction_transaction.sql
//!
//! When we put settlement transactions on chain there is no reliable way to know the transaction
//! hash because we can create multiple transactions with different gas prices. What we do know is
//! the account and nonce that the transaction will have which is enough to uniquely identify it.
//!
//! We build an association between account-nonce and tx hash by backfilling settlement events with
//! the account and nonce of their tx hash. This happens in an always running background task.
//!
//! Alternatively we could change the event insertion code to do this but I (vk) would like to keep
//! that code as fast as possible to not slow down event insertion which also needs to deal with
//! reorgs. It is also nicer from a code organization standpoint.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use primitive_types::H256;
use shared::{
    current_block::CurrentBlockStream, ethrpc::Web3, event_handling::MAX_REORG_BLOCK_COUNT,
};
use web3::types::TransactionId;

use crate::database::Postgres;

pub struct AuctionTransactionUpdater {
    pub web3: Web3,
    pub db: Postgres,
    pub current_block: CurrentBlockStream,
}

impl AuctionTransactionUpdater {
    pub async fn run_forever(self) -> ! {
        loop {
            match self.update().await {
                Ok(true) => (),
                Ok(false) => tokio::time::sleep(Duration::from_secs(10)).await,
                Err(err) => {
                    tracing::error!(?err, "auction transaction update task failed");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// Update a single settlement event.
    ///
    /// Returns whether an update was performed.
    async fn update(&self) -> Result<bool> {
        let current_block = self
            .current_block
            .borrow()
            .number
            .context("no block number")?
            .as_u64();
        let reorg_safe_block: u64 = current_block
            .checked_sub(MAX_REORG_BLOCK_COUNT)
            .context("no reorg safe block")?;
        let reorg_safe_block: i64 = reorg_safe_block.try_into().context("convert block")?;
        let event = match self
            .db
            .get_settlement_event_without_tx_info(reorg_safe_block)
            .await
            .context("get_settlement_event_without_tx_info")?
        {
            Some(event) => event,
            None => return Ok(false),
        };
        let hash = H256(event.tx_hash.0);
        tracing::trace!(?hash);

        let transaction = self
            .web3
            .eth()
            .transaction(TransactionId::Hash(hash))
            .await
            .with_context(|| format!("get tx {hash:?}"))?
            .with_context(|| format!("no tx {hash:?}"))?;
        let from = transaction
            .from
            .with_context(|| format!("no from {hash:?}"))?;
        let nonce: i64 = transaction
            .nonce
            .try_into()
            .map_err(|err| anyhow!("{}", err))
            .with_context(|| format!("convert nonce {hash:?}"))?;

        self.db
            .update_settlement_tx_info(event.block_number, event.log_index, from, nonce, hash)
            .await
            .context("update_settlement_tx_info")?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use sqlx::Executor;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn manual_node_test() {
        shared::tracing::initialize_for_tests("autopilot=trace");
        let db = Postgres::new("postgresql://").await.unwrap();
        database::clear_DANGER(&db.0).await.unwrap();
        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let current_block =
            shared::current_block::current_block_stream(web3.clone(), Duration::from_secs(1))
                .await
                .unwrap();
        let updater = AuctionTransactionUpdater {
            web3,
            db,
            current_block,
        };

        assert!(!updater.update().await.unwrap());

        let query = r#"
INSERT INTO settlements (block_number, log_index, solver, tx_hash, tx_from, tx_nonce)
VALUES (15875801, 405, '\x', '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a', NULL, NULL)
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        let query = r#"
INSERT INTO solver_competitions (id, tx_hash)
VALUES (0, '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a')
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        assert!(updater.update().await.unwrap());

        let query = r#"
SELECT tx_from, tx_nonce
FROM settlements
WHERE block_number = 15875801 AND log_index = 405
        ;"#;
        let (tx_from, tx_nonce): (Vec<u8>, i64) = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(
            tx_from,
            hex_literal::hex!("a21740833858985e4d801533a808786d3647fb83")
        );
        assert_eq!(tx_nonce, 4701);

        let query = r#"
SELECT auction_id, tx_from, tx_nonce
FROM auction_transaction
        ;"#;
        let (auction_id, tx_from, tx_nonce): (i64, Vec<u8>, i64) = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(auction_id, 0);
        assert_eq!(
            tx_from,
            hex_literal::hex!("a21740833858985e4d801533a808786d3647fb83")
        );
        assert_eq!(tx_nonce, 4701);

        assert!(!updater.update().await.unwrap());
    }
}
