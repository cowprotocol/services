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
            on_settlement_event_updater::{AuctionData, AuctionId, SettlementUpdate},
            Postgres,
        },
        decoded_settlement::{DecodedSettlement, DecodingError},
    },
    anyhow::{anyhow, Context, Result},
    contracts::GPv2Settlement,
    database::byte_array::ByteArray,
    ethrpc::{
        current_block::{into_stream, CurrentBlockStream},
        Web3,
    },
    futures::StreamExt,
    model::DomainSeparator,
    primitive_types::{H160, H256},
    shared::{event_handling::MAX_REORG_BLOCK_COUNT, external_prices::ExternalPrices},
    sqlx::PgConnection,
    web3::types::{Transaction, TransactionId},
};

pub struct OnSettlementEventUpdater {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub native_token: H160,
    pub db: Postgres,
}

impl OnSettlementEventUpdater {
    pub async fn run_forever(self, block_stream: CurrentBlockStream) -> ! {
        let mut current_block = *block_stream.borrow();
        let mut block_stream = into_stream(block_stream);
        loop {
            match self.update(current_block.number).await {
                Ok(true) => {
                    tracing::debug!(
                        block = current_block.number,
                        "on settlement event updater ran and processed event"
                    );
                    // Don't wait until next block in case there are more pending events to process.
                    continue;
                }
                Ok(false) => {
                    tracing::debug!(
                        block = current_block.number,
                        "on settlement event updater ran without update"
                    );
                }
                Err(err) => {
                    tracing::error!(?err, "on settlement event update task failed");
                }
            }
            current_block = block_stream.next().await.expect("blockchains never end");
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn update(&self, current_block: u64) -> Result<bool> {
        let reorg_safe_block: i64 = current_block
            .checked_sub(MAX_REORG_BLOCK_COUNT)
            .context("no reorg safe block")?
            .try_into()
            .context("convert block")?;

        let mut ex = self.db.0.begin().await.context("acquire DB connection")?;
        let event = match database::auction_transaction::get_settlement_event_without_tx_info(
            &mut ex,
            reorg_safe_block,
        )
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

        let domain_separator = DomainSeparator(self.contract.domain_separator().call().await?.0);

        let mut auction_id =
            Self::recover_auction_id_from_calldata(&mut ex, &transaction, &domain_separator)
                .await?
                .map(AuctionId::Colocated);
        if auction_id.is_none() {
            // This settlement was issued BEFORE solver-driver colocation.
            auction_id = database::auction_transaction::get_auction_id(
                &mut ex,
                &ByteArray(tx_from.0),
                tx_nonce,
            )
            .await?
            .map(AuctionId::Centralized);
        }

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
            let auction_external_prices =
                Postgres::get_auction_prices(&mut ex, auction_id.assume_verified())
                    .await
                    .with_context(|| {
                        format!("no external prices for auction id {auction_id:?} and tx {hash:?}")
                    })?;
            let orders =
                Postgres::order_executions_for_tx(&mut ex, &hash, auction_id.assume_verified())
                    .await?;
            let external_prices = ExternalPrices::try_from_auction_prices(
                self.native_token,
                auction_external_prices.clone(),
            )?;

            tracing::debug!(
                ?auction_id,
                ?auction_external_prices,
                ?orders,
                ?external_prices,
                "observations input"
            );

            // surplus and fees calculation
            match DecodedSettlement::new(&transaction.input.0, &domain_separator) {
                Ok(settlement) => {
                    let surplus = settlement.total_surplus(&external_prices);
                    let fee = settlement.total_fees(&external_prices, orders.clone());
                    let order_executions = settlement.order_executions(&external_prices, orders);

                    update.auction_data = Some(AuctionData {
                        auction_id,
                        surplus,
                        fee,
                        gas_used,
                        effective_gas_price,
                        order_executions: order_executions
                            .iter()
                            .map(|fees| (fees.order, fees.sell))
                            .collect(),
                    });
                }
                Err(DecodingError::InvalidSelector) => {
                    // we indexed a transaction initiated by solver, that was not a settlement
                    // for this case we want to have the entry in observations table but with zeros
                    update.auction_data = Some(Default::default());
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        tracing::debug!(?hash, ?update, "updating settlement details for tx");

        Postgres::update_settlement_details(&mut ex, update.clone())
            .await
            .with_context(|| format!("insert_settlement_details: {update:?}"))?;
        ex.commit().await?;
        Ok(true)
    }

    /// With solver driver colocation solvers are supposed to append the
    /// `auction_id` to the settlement calldata. This function tries to
    /// recover that `auction_id`. This function only returns an error if
    /// retrying the operation makes sense. If all went well and there
    /// simply is no sensible `auction_id` `Ok(None)` will be returned.
    async fn recover_auction_id_from_calldata(
        ex: &mut PgConnection,
        tx: &Transaction,
        domain_separator: &DomainSeparator,
    ) -> Result<Option<i64>> {
        let tx_from = tx.from.context("tx is missing sender")?;
        let metadata = match DecodedSettlement::new(&tx.input.0, domain_separator) {
            Ok(settlement) => settlement.metadata,
            Err(err) => {
                tracing::warn!(
                    ?tx,
                    ?err,
                    "could not decode settlement tx, unclear which auction it belongs to"
                );
                return Ok(None);
            }
        };
        let auction_id = match metadata {
            Some(bytes) => i64::from_be_bytes(bytes.0),
            None => {
                tracing::warn!(?tx, "could not recover the auction_id from the calldata");
                return Ok(None);
            }
        };

        let score = database::settlement_scores::fetch(ex, auction_id).await?;
        let data_already_recorded =
            database::auction_transaction::data_exists(ex, auction_id).await?;
        match (score, data_already_recorded) {
            (None, _) => {
                tracing::debug!(
                    auction_id,
                    "calldata claims to settle auction that has no competition"
                );
                Ok(None)
            }
            (Some(score), _) if score.winner.0 != tx_from.0 => {
                tracing::warn!(
                    auction_id,
                    ?tx_from,
                    winner = ?score.winner,
                    "solution submitted by solver other than the winner"
                );
                Ok(None)
            }
            (Some(_), true) => {
                tracing::warn!(
                    auction_id,
                    "settlement data already recorded for this auction"
                );
                Ok(None)
            }
            (Some(_), false) => Ok(Some(auction_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        database::{auction_prices::AuctionPrice, settlement_observations::Observation},
        sqlx::Executor,
    };

    #[tokio::test]
    #[ignore]
    async fn manual_node_test() {
        // TODO update test
        observe::tracing::initialize_reentrant("autopilot=trace");
        let db = Postgres::new("postgresql://").await.unwrap();
        database::clear_DANGER(&db.0).await.unwrap();
        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);

        let contract = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let updater = OnSettlementEventUpdater {
            web3,
            db,
            native_token,
            contract,
        };

        assert!(!updater.update(15875900).await.unwrap());

        let query = r"
INSERT INTO settlements (block_number, log_index, solver, tx_hash, tx_from, tx_nonce)
VALUES (15875801, 405, '\x', '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a', NULL, NULL)
    ;";
        updater.db.0.execute(query).await.unwrap();

        let query = r"
INSERT INTO auction_transaction (auction_id, tx_from, tx_nonce)
VALUES (0, '\xa21740833858985e4d801533a808786d3647fb83', 4701)
    ;";
        updater.db.0.execute(query).await.unwrap();

        let query = r"
INSERT INTO auction_prices (auction_id, token, price)
VALUES (0, '\x056fd409e1d7a124bd7017459dfea2f387b6d5cd', 6347795727933475088343330979840),
        (0, '\x6b175474e89094c44da98b954eedeac495271d0f', 634671683530053),
        (0, '\xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 634553336916241343152390144)
            ;";

        updater.db.0.execute(query).await.unwrap();

        assert!(updater.update(15875900).await.unwrap());

        let query = r"
SELECT tx_from, tx_nonce
FROM settlements
WHERE block_number = 15875801 AND log_index = 405
        ;";
        let (tx_from, tx_nonce): (Vec<u8>, i64) = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(
            tx_from,
            hex_literal::hex!("a21740833858985e4d801533a808786d3647fb83")
        );
        assert_eq!(tx_nonce, 4701);

        let query = r"
SELECT auction_id, tx_from, tx_nonce
FROM auction_transaction
        ;";
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

        assert!(!updater.update(15875900).await.unwrap());
    }
}
