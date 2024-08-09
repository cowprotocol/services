//! This module is responsible for updating the database, for each settlement
//! event that is emitted by the settlement contract.
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
        decoded_settlement::DecodedSettlement,
        domain::{self, settlement::Transaction},
        infra,
    },
    anyhow::{Context, Result},
    database::{surplus_capturing_jit_order_owners, PgTransaction},
    primitive_types::{H160, H256},
    shared::external_prices::ExternalPrices,
    sqlx::PgConnection,
    std::{collections::HashSet, sync::Arc},
    tokio::sync::Notify,
};

pub struct OnSettlementEventUpdater {
    inner: Arc<Inner>,
}

struct Inner {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    db: Postgres,
    notify: Notify,
}

enum AuctionIdRecoveryStatus {
    /// The auction id was recovered and the auction data should be added.
    AddAuctionData(i64, DecodedSettlement),
    /// The auction id was recovered but the auction data should not be added.
    DoNotAddAuctionData(i64),
    /// The auction id was not recovered.
    InvalidCalldata,
}

impl OnSettlementEventUpdater {
    /// Creates a new OnSettlementEventUpdater and asynchronously schedules the
    /// first update run.
    pub fn new(eth: infra::Ethereum, db: Postgres, persistence: infra::Persistence) -> Self {
        let inner = Arc::new(Inner {
            eth,
            persistence,
            db,
            notify: Notify::new(),
        });
        let inner_clone = inner.clone();
        tokio::spawn(async move { Inner::listen_for_updates(inner_clone).await });
        Self { inner }
    }

    /// Deletes settlement_observations and order executions for the given range
    pub async fn delete_observations(
        transaction: &mut PgTransaction<'_>,
        from_block: u64,
    ) -> Result<()> {
        database::settlements::delete(transaction, from_block)
            .await
            .context("delete_settlement_observations")?;

        Ok(())
    }

    /// Schedules an update loop on a background thread
    pub fn schedule_update(&self) {
        self.inner.notify.notify_one();
    }
}

impl Inner {
    async fn listen_for_updates(self: Arc<Inner>) -> ! {
        loop {
            match self.update().await {
                Ok(true) => {
                    tracing::debug!("on settlement event updater ran and processed event");
                    // There might be more pending updates, continue immediately.
                    continue;
                }
                Ok(false) => {
                    tracing::debug!("on settlement event updater ran without update");
                }
                Err(err) => {
                    tracing::error!(?err, "on settlement event update task failed");
                }
            }
            self.notify.notified().await;
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

        let Ok(transaction) = self.eth.transaction(hash.into()).await else {
            tracing::warn!(?hash, "no tx found");
            return Ok(false);
        };
        let domain_separator = self.eth.contracts().settlement_domain_separator();

        let (auction_id, auction_data) = match Self::recover_auction_id_from_calldata(
            &mut ex,
            &transaction,
            &model::DomainSeparator(domain_separator.0),
        )
        .await?
        {
            AuctionIdRecoveryStatus::InvalidCalldata => {
                // To not get stuck on indexing the same transaction over and over again, we
                // insert the default auction ID (0)
                (Default::default(), None)
            }
            AuctionIdRecoveryStatus::DoNotAddAuctionData(auction_id) => (auction_id, None),
            AuctionIdRecoveryStatus::AddAuctionData(auction_id, settlement) => (
                auction_id,
                Some(
                    self.fetch_auction_data(settlement, auction_id, &transaction, &mut ex)
                        .await?,
                ),
            ),
        };

        let update = SettlementUpdate {
            block_number: event.block_number,
            log_index: event.log_index,
            auction_id,
            auction_data: auction_data.clone(),
        };

        tracing::debug!(?hash, ?update, "updating settlement details for tx");

        {
            // temporary to debug and compare with current implementation
            let settlement = domain::settlement::Settlement::new(
                transaction,
                domain_separator,
                &self.persistence,
            )
            .await;

            // automatic checks vs current implementation
            match (settlement, auction_data) {
                (Ok(_), None) => {
                    // bug: we should have an auction_data
                    tracing::warn!(?auction_id, "automatic check error: missing auction_data");
                }
                (Ok(settlement), Some(auction_data)) => {
                    // staging settlement properly built
                    let surplus = settlement.native_surplus();
                    if surplus.0 != auction_data.surplus {
                        tracing::warn!(
                            ?auction_id,
                            ?surplus,
                            ?auction_data.surplus,
                            "automatic check error: surplus mismatch"
                        );
                    }
                    let fee = settlement.native_fee();
                    if fee.0 != auction_data.fee {
                        tracing::warn!(
                            ?auction_id,
                            ?fee,
                            ?auction_data.fee,
                            "automatic check error: fee mismatch"
                        );
                    }
                    let order_fees = settlement.order_fees();
                    if order_fees.len() != auction_data.order_executions.len() {
                        tracing::warn!(
                            ?auction_id,
                            ?order_fees,
                            ?auction_data.order_executions,
                            "automatic check error: order_fees mismatch"
                        );
                    }
                    for fee in auction_data.order_executions {
                        if !order_fees.contains_key(&domain::OrderUid(fee.0 .0)) {
                            tracing::warn!(
                                ?auction_id,
                                ?fee,
                                ?order_fees,
                                "automatic check error: order_fees missing"
                            );
                        } else {
                            let settlement_fee = order_fees[&domain::OrderUid(fee.0 .0)];
                            if settlement_fee.unwrap_or_default().0 != fee.1 {
                                tracing::warn!(
                                    ?auction_id,
                                    ?settlement_fee,
                                    ?fee,
                                    "automatic check error: order_fees value mismatch"
                                );
                            }
                        }
                    }
                }
                (Err(err), None) => {
                    // make sure the auction_ids are equal
                    if err.auction_id.unwrap_or_default() != auction_id {
                        tracing::warn!(
                            ?auction_id,
                            ?err,
                            "automatic check error: auction_id mismatch"
                        );
                    }
                }
                (Err(err), Some(_)) => {
                    // bug: settlement should have been properly built
                    tracing::warn!(
                        ?auction_id,
                        ?err,
                        "automatic check error: settlement error for valid auction_data"
                    );
                }
            }
        }

        Postgres::update_settlement_details(&mut ex, update.clone())
            .await
            .with_context(|| format!("insert_settlement_details: {update:?}"))?;
        ex.commit().await?;

        Ok(true)
    }

    async fn fetch_auction_data(
        &self,
        settlement: DecodedSettlement,
        auction_id: i64,
        tx: &Transaction,
        ex: &mut PgConnection,
    ) -> Result<AuctionData> {
        let auction = Postgres::find_competition(auction_id, ex)
            .await?
            .context(format!(
                "missing competition for auction_id={:?}",
                auction_id
            ))?
            .common
            .auction;
        let external_prices = ExternalPrices::try_from_auction_prices(
            self.eth.contracts().weth().address(),
            auction.prices.clone(),
        )?;
        let surplus_capturing_jit_order_owners =
            surplus_capturing_jit_order_owners::fetch(ex, auction_id)
                .await?
                .unwrap_or_default()
                .into_iter()
                .map(|owner| H160(owner.0))
                .collect::<HashSet<_>>();

        tracing::debug!(
            ?auction_id,
            auction_external_prices=?auction.prices,
            ?external_prices,
            "observations input"
        );

        // surplus and fees calculation
        let surplus = settlement.total_surplus(
            &external_prices,
            &auction.orders.into_iter().collect::<HashSet<_>>(),
            &surplus_capturing_jit_order_owners,
        );
        let (fee, order_executions) = {
            let all_fees = settlement.all_fees(&external_prices);
            // total fee used for CIP20 rewards
            let fee = all_fees
                .iter()
                .fold(0.into(), |acc, fees| acc + fees.native);
            // executed surplus fees for each order execution
            let order_executions = all_fees
                .into_iter()
                .map(|fee| (fee.order, fee.executed_surplus_fee().unwrap_or(0.into())))
                .collect();
            (fee, order_executions)
        };

        Ok(AuctionData {
            surplus,
            fee,
            gas_used: tx.gas.into(),
            effective_gas_price: tx.effective_gas_price.into(),
            order_executions,
        })
    }

    /// With solver driver colocation solvers are supposed to append the
    /// `auction_id` to the settlement calldata. This function tries to
    /// recover that `auction_id`. It also indicates whether the auction
    /// should be indexed with its metadata. (ie. if it comes from this
    /// environment and not from a different instance of the autopilot, e.g.
    /// running in barn/prod). This function only returns an error
    /// if retrying the operation makes sense.
    async fn recover_auction_id_from_calldata(
        ex: &mut PgConnection,
        tx: &Transaction,
        domain_separator: &model::DomainSeparator,
    ) -> Result<AuctionIdRecoveryStatus> {
        let tx_from = tx.solver.0;
        let settlement = match DecodedSettlement::new(&tx.input.0, domain_separator) {
            Ok(settlement) => settlement,
            Err(err) => {
                tracing::warn!(
                    ?tx,
                    ?err,
                    "could not decode settlement tx, unclear which auction it belongs to"
                );
                return Ok(AuctionIdRecoveryStatus::InvalidCalldata);
            }
        };
        let auction_id = match settlement.metadata {
            Some(bytes) => i64::from_be_bytes(bytes.0),
            None => {
                tracing::warn!(?tx, "could not recover the auction_id from the calldata");
                return Ok(AuctionIdRecoveryStatus::InvalidCalldata);
            }
        };

        let score = database::settlement_scores::fetch(ex, auction_id).await?;
        let data_already_recorded =
            database::settlements::already_processed(ex, auction_id).await?;
        match (score, data_already_recorded) {
            (None, _) => {
                tracing::debug!(
                    auction_id,
                    "calldata claims to settle auction that has no competition"
                );
                Ok(AuctionIdRecoveryStatus::DoNotAddAuctionData(auction_id))
            }
            (Some(_), true) => {
                tracing::warn!(
                    auction_id,
                    "settlement data already recorded for this auction"
                );
                Ok(AuctionIdRecoveryStatus::DoNotAddAuctionData(auction_id))
            }
            (Some(score), _) if score.winner.0 != tx_from.0 => {
                tracing::warn!(
                    auction_id,
                    ?tx_from,
                    winner = ?score.winner,
                    "solution submitted by solver other than the winner"
                );
                Ok(AuctionIdRecoveryStatus::DoNotAddAuctionData(auction_id))
            }
            (Some(_), false) => Ok(AuctionIdRecoveryStatus::AddAuctionData(
                auction_id, settlement,
            )),
        }
    }
}
