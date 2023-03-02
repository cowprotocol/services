//! This module is responsible for updating the database, for each settlement
//! event that is emitted by the settlement contract.
//!
//! 1. Associating auction ids with transaction hashes.
// see database/sql/V037__auction_transaction.sql
//
// When we put settlement transactions on chain there is no reliable way to
// know the transaction hash because we can create multiple transactions with
// different gas prices. What we do know is the account and nonce that the
// transaction will have which is enough to uniquely identify it.
//
// We build an association between account-nonce and tx hash by backfilling
// settlement events with the account and nonce of their tx hash. This happens
// in an always running background task.
//
// Alternatively we could change the event insertion code to do this but I (vk)
// would like to keep that code as fast as possible to not slow down event
// insertion which also needs to deal with reorgs. It is also nicer from a code
// organization standpoint.

// 2. Inserting settlement observations
//
// see database/sql/V048__create_settlement_rewards.sql
//
// Surplus and fees calculation is based on:
// a) the mined transaction call data
// b) the auction external prices fetched from orderbook
// c) the orders fetched from orderbook
// After a transaction is mined we calculate the surplus and fees for each
// transaction and insert them into the database (settlement_observations
// table).

use {
    crate::{
        database::{
            on_settlement_event_updater::{AuctionData, SettlementUpdate},
            Postgres,
        },
        decoded_settlement::{DecodedSettlement, FeeConfiguration},
    },
    anyhow::{anyhow, Context, Result},
    contracts::GPv2Settlement,
    num::BigRational,
    primitive_types::{H160, H256},
    shared::{
        current_block::CurrentBlockStream,
        ethrpc::Web3,
        event_handling::MAX_REORG_BLOCK_COUNT,
        external_prices::ExternalPrices,
    },
    std::time::Duration,
    web3::types::TransactionId,
};

pub struct OnSettlementEventUpdater {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub native_token: H160,
    pub db: Postgres,
    pub current_block: CurrentBlockStream,
    pub fee_objective_scaling_factor: BigRational,
}

impl OnSettlementEventUpdater {
    pub async fn run_forever(self) -> ! {
        loop {
            match self.update().await {
                Ok(true) => (),
                Ok(false) => tokio::time::sleep(Duration::from_secs(10)).await,
                Err(err) => {
                    tracing::error!(?err, "on settlement event update task failed");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn update(&self) -> Result<bool> {
        let current_block = self.current_block.borrow().number;
        let reorg_safe_block: i64 = current_block
            .checked_sub(MAX_REORG_BLOCK_COUNT)
            .context("no reorg safe block")?
            .try_into()
            .context("convert block")?;

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
        tracing::debug!("updating settlement details for tx {hash:?}");

        let transaction = self
            .web3
            .eth()
            .transaction(TransactionId::Hash(hash))
            .await
            .with_context(|| format!("get tx {hash:?}"))?
            .with_context(|| format!("no tx {hash:?}"))?;
        let tx_from = transaction
            .from
            .with_context(|| format!("no from {hash:?}"))?;
        let tx_nonce: i64 = transaction
            .nonce
            .try_into()
            .map_err(|err| anyhow!("{}", err))
            .with_context(|| format!("convert nonce {hash:?}"))?;

        let auction_id = self.db.get_auction_id(tx_from, tx_nonce).await?;
        let mut update = SettlementUpdate {
            block_number: event.block_number,
            log_index: event.log_index,
            tx_from,
            tx_nonce,
            auction_data: None,
        };

        // It is possible that auction_id does not exist for a transaction.
        // This happens for production auctions queried from the staging environment and
        // vice versa (because we have databases for both environments).
        //
        // If auction_id exists, we expect all other relevant information to exist as
        // well.
        if let Some(auction_id) = auction_id {
            let receipt = self
                .web3
                .eth()
                .transaction_receipt(hash)
                .await?
                .with_context(|| format!("no receipt {hash:?}"))?;
            let gas_used = receipt
                .gas_used
                .with_context(|| format!("no gas used {hash:?}"))?;
            let effective_gas_price = receipt
                .effective_gas_price
                .with_context(|| format!("no effective gas price {hash:?}"))?;
            let auction_external_prices = self
                .db
                .get_auction_prices(auction_id)
                .await
                .with_context(|| {
                    format!("no external prices for auction id {auction_id:?} and tx {hash:?}")
                })?;
            let orders = self
                .db
                .orders_for_tx(&hash)
                .await?
                .into_iter()
                .map(|order| {
                    order.try_into().with_context(|| {
                        format!(
                            "failed to convert order for tx {hash:?} and auction id {auction_id:?}"
                        )
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let external_prices = ExternalPrices::try_from_auction_prices(
                self.native_token,
                auction_external_prices.clone(),
            )?;

            // surplus and fees calculation
            let configuration = FeeConfiguration {
                fee_objective_scaling_factor: self.fee_objective_scaling_factor.clone(),
            };
            let settlement = DecodedSettlement::new(&transaction.input.0)?;
            let surplus = settlement.total_surplus(&external_prices);
            let fee = settlement.total_fees(&external_prices, &orders, &configuration);

            update.auction_data = Some(AuctionData {
                surplus,
                fee,
                gas_used,
                effective_gas_price,
            });

            tracing::trace!(
                ?auction_id,
                ?auction_external_prices,
                ?orders,
                ?external_prices,
                "observations input"
            );
        }

        tracing::debug!(?hash, ?update, "updating settlement details for tx");

        self.db
            .update_settlement_details(update.clone())
            .await
            .with_context(|| format!("insert_settlement_details: {update:?}"))?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        bigdecimal::One,
        database::{auction_prices::AuctionPrice, settlement_observations::Observation},
        sqlx::Executor,
        std::sync::Arc,
    };

    #[tokio::test]
    #[ignore]
    async fn manual_node_test() {
        // TODO update test
        shared::tracing::initialize_reentrant("autopilot=trace");
        let db = Postgres::new("postgresql://").await.unwrap();
        database::clear_DANGER(&db.0).await.unwrap();
        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let current_block = shared::current_block::current_block_stream(
            Arc::new(web3.clone()),
            Duration::from_secs(1),
        )
        .await
        .unwrap();

        let contract = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let updater = OnSettlementEventUpdater {
            web3,
            db,
            native_token,
            current_block,
            contract,
            fee_objective_scaling_factor: BigRational::one(),
        };

        assert!(!updater.update().await.unwrap());

        let query = r#"
INSERT INTO settlements (block_number, log_index, solver, tx_hash, tx_from, tx_nonce)
VALUES (15875801, 405, '\x', '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a', NULL, NULL)
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        let query = r#"
INSERT INTO auction_transaction (auction_id, tx_from, tx_nonce)
VALUES (0, '\xa21740833858985e4d801533a808786d3647fb83', 4701)
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        let query = r#"
INSERT INTO auction_prices (auction_id, token, price)
VALUES (0, '\x056fd409e1d7a124bd7017459dfea2f387b6d5cd', 6347795727933475088343330979840),
        (0, '\x6b175474e89094c44da98b954eedeac495271d0f', 634671683530053),
        (0, '\xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 634553336916241343152390144)
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

        // assert that the prices are updated
        let query = r#"SELECT * FROM auction_prices;"#;
        let prices: Vec<AuctionPrice> = sqlx::query_as(query)
            .fetch_all(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(prices.len(), 2);

        // assert that the observations are updated
        let query = r#"SELECT * FROM settlement_observations;"#;
        let observation: Observation = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(observation.gas_used, 179155.into());
        assert_eq!(observation.effective_gas_price, 19789368758u64.into());
        assert_eq!(observation.surplus, 5150444803867862u64.into());

        assert!(!updater.update().await.unwrap());
    }
}
