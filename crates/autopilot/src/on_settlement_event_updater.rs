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
        decoded_settlement::{DecodedSettlement, DecodingError},
        infra,
    },
    anyhow::{Context, Result},
    futures::StreamExt,
    primitive_types::H256,
    shared::external_prices::ExternalPrices,
    sqlx::PgConnection,
    web3::types::Transaction,
};

#[derive(Debug, Copy, Clone)]
pub enum AuctionKind {
    /// This auction is regular and all the auction dependent data should be
    /// updated.
    Valid { auction_id: i64 },
    /// Some possible reasons to have invalid auction are:
    /// - This auction was created by another environment (e.g.
    ///   production/staging)
    /// - Failed to decode settlement calldata
    /// - Failed to recover auction id from calldata
    /// - Settlement transaction was submitted by solver other than the winner
    ///
    /// In this case, settlement event should be marked as invalid and no
    /// auction dependent data is updated.
    Invalid,
}

impl AuctionKind {
    pub fn auction_id(&self) -> Option<i64> {
        match self {
            AuctionKind::Valid { auction_id } => Some(*auction_id),
            AuctionKind::Invalid => None,
        }
    }
}

pub struct OnSettlementEventUpdater {
    pub eth: infra::Ethereum,
    pub db: Postgres,
}

impl OnSettlementEventUpdater {
    pub async fn run_forever(self) -> ! {
        let mut current_block = self.eth.current_block().borrow().to_owned();
        let mut block_stream = ethrpc::current_block::into_stream(self.eth.current_block().clone());
        loop {
            match self.update().await {
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
    async fn update(&self) -> Result<bool> {
        let mut ex = self
            .db
            .pool
            .begin()
            .await
            .context("acquire DB connection")?;
        let event = match database::settlements::get_settlement_without_auction(&mut ex)
            .await
            .context("get_settlement_without_auction")?
        {
            Some(event) => event,
            None => return Ok(false),
        };

        let hash = H256(event.tx_hash.0);
        tracing::debug!("updating settlement details for tx {hash:?}");

        let transaction = self
            .eth
            .transaction(hash)
            .await?
            .with_context(|| format!("no tx {hash:?}"))?;

        let auction_kind = Self::get_auction_kind(&mut ex, &transaction).await?;

        let mut update = SettlementUpdate {
            block_number: event.block_number,
            log_index: event.log_index,
            auction_kind,
            auction_data: None,
        };

        // It is possible that auction_id does not exist for a transaction.
        // This happens for production auctions queried from the staging environment and
        // vice versa (because we have databases for both environments).
        //
        // If auction_id exists, we expect all other relevant information to exist as
        // well.
        if let AuctionKind::Valid { auction_id } = auction_kind {
            let receipt = self
                .eth
                .transaction_receipt(hash)
                .await?
                .with_context(|| format!("no receipt {hash:?}"))?;
            let gas_used = receipt
                .gas_used
                .with_context(|| format!("no gas used {hash:?}"))?;
            let effective_gas_price = receipt
                .effective_gas_price
                .with_context(|| format!("no effective gas price {hash:?}"))?;
            let auction_external_prices = Postgres::get_auction_prices(&mut ex, auction_id)
                .await
                .with_context(|| {
                format!("no external prices for auction id {auction_id:?} and tx {hash:?}")
            })?;
            let external_prices = ExternalPrices::try_from_auction_prices(
                self.eth.contracts().weth().address(),
                auction_external_prices.clone(),
            )?;

            tracing::debug!(
                ?auction_id,
                ?auction_external_prices,
                ?external_prices,
                "observations input"
            );

            // surplus and fees calculation
            match DecodedSettlement::new(&transaction.input.0) {
                Ok(settlement) => {
                    let domain_separator = self.eth.contracts().settlement_domain_separator();
                    let order_uids = settlement.order_uids(domain_separator)?;
                    let order_fees = order_uids
                        .clone()
                        .into_iter()
                        .zip(Postgres::order_fees(&mut ex, &order_uids).await?)
                        .collect::<Vec<_>>();

                    let surplus = settlement.total_surplus(&external_prices);
                    let (fee, order_executions) = {
                        let all_fees = settlement.all_fees(&external_prices, &order_fees);
                        // total unsubsidized fee used for CIP20 rewards
                        let fee = all_fees
                            .iter()
                            .fold(0.into(), |acc, fees| acc + fees.native);
                        // executed fees for each order execution
                        let order_executions = all_fees
                            .into_iter()
                            .zip(order_fees.iter())
                            .filter_map(|(fee, (_, order_fee))| match order_fee {
                                // filter out orders with order_fee
                                // since their fee can already be fetched from the database table
                                // `orders` so no point in storing the same
                                // value twice, in another table
                                Some(_) => None,
                                None => Some((fee.order, fee.sell)),
                            })
                            .collect();
                        (fee, order_executions)
                    };

                    update.auction_data = Some(AuctionData {
                        auction_id,
                        surplus,
                        fee,
                        gas_used,
                        effective_gas_price,
                        order_executions,
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
    /// simply is no sensible `auction_id` `AuctionKind::Invalid` will be
    /// returned.
    async fn get_auction_kind(ex: &mut PgConnection, tx: &Transaction) -> Result<AuctionKind> {
        let tx_from = tx.from.context("tx is missing sender")?;
        let metadata = match DecodedSettlement::new(&tx.input.0) {
            Ok(settlement) => settlement.metadata,
            Err(err) => {
                tracing::warn!(
                    ?tx,
                    ?err,
                    "could not decode settlement tx, unclear which auction it belongs to"
                );
                return Ok(AuctionKind::Invalid);
            }
        };
        let auction_id = match metadata {
            Some(bytes) => i64::from_be_bytes(bytes.0),
            None => {
                tracing::warn!(?tx, "could not recover the auction_id from the calldata");
                return Ok(AuctionKind::Invalid);
            }
        };

        let score = database::settlement_scores::fetch(ex, auction_id).await?;
        match score {
            None => {
                tracing::debug!(
                    auction_id,
                    "calldata claims to settle auction that has no competition"
                );
                Ok(AuctionKind::Invalid)
            }
            Some(score) => {
                if score.winner.0 != tx_from.0 {
                    tracing::warn!(
                        auction_id,
                        ?tx_from,
                        winner = ?score.winner,
                        "solution submitted by solver other than the winner"
                    );
                    Ok(AuctionKind::Invalid)
                } else {
                    Ok(AuctionKind::Valid { auction_id })
                }
            }
        }
    }
}
