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
        domain,
        infra,
    },
    anyhow::{Context, Result},
    database::PgTransaction,
    primitive_types::H256,
    shared::external_prices::ExternalPrices,
    sqlx::PgConnection,
    std::sync::Arc,
    tokio::sync::Notify,
};

pub struct OnSettlementEventUpdater {
    inner: Arc<Inner>,
}

struct Inner {
    eth: infra::Ethereum,
    db: Postgres,
    notify: Notify,
}

impl OnSettlementEventUpdater {
    /// Creates a new OnSettlementEventUpdater and asynchronously schedules the
    /// first update run.
    pub fn new(eth: infra::Ethereum, db: Postgres) -> Self {
        let inner = Arc::new(Inner {
            eth,
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

        let settlement = domain::Settlement::new(hash.into(), self.eth.clone()).await;
        let update = match settlement {
            Ok(settlement) => SettlementUpdate {
                block_number: event.block_number,
                log_index: event.log_index,
                auction_id: settlement.auction_id(),
                auction_data: Some(self.fetch_auction_data(settlement, &mut ex).await?),
            },
            Err(err) => match err {
                domain::settlement::Error::Blockchain(err) => return Err(err.into()),
                domain::settlement::Error::TransactionNotFound => {
                    tracing::warn!(?hash, "settlement tx not found, reorg happened");
                    return Ok(false);
                }
                domain::settlement::Error::Encoded(err) => {
                    tracing::warn!(?err, "could not decode settlement tx");
                    SettlementUpdate {
                        block_number: event.block_number,
                        log_index: event.log_index,
                        auction_id: 0,
                        auction_data: None,
                    }
                }
            },
        };

        tracing::debug!(?hash, ?update, "updating settlement details for tx");

        Postgres::update_settlement_details(&mut ex, update.clone())
            .await
            .with_context(|| format!("insert_settlement_details: {update:?}"))?;
        ex.commit().await?;
        Ok(true)
    }

    async fn fetch_auction_data(
        &self,
        settlement: domain::Settlement,
        ex: &mut PgConnection,
    ) -> Result<AuctionData> {
        let auction_id = settlement.auction_id();
        let hash = settlement.transaction().hash();
        let auction_external_prices = Postgres::get_auction_prices(ex, auction_id)
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
        let _surplus = domain::settlement::Surplus::new(&settlement.encoded());
        // surplus and fees calculation
        // let surplus = settlement.total_surplus(&external_prices);
        // let (fee, order_executions) = {
        //     let domain_separator =
        // self.eth.contracts().settlement_domain_separator();     let all_fees
        // = settlement.all_fees(&external_prices, domain_separator);     // total
        // fee used for CIP20 rewards     let fee = all_fees
        //         .iter()
        //         .fold(0.into(), |acc, fees| acc + fees.native);
        //     // executed surplus fees for each order execution
        //     let order_executions = all_fees
        //         .into_iter()
        //         .map(|fee| (fee.order,
        // fee.executed_surplus_fee().unwrap_or(0.into())))         .collect();
        //     (fee, order_executions)
        // };

        // Ok(AuctionData {
        //     surplus,
        //     fee,
        //     gas_used: settlement.transaction_receipt().gas(),
        //     effective_gas_price:
        // settlement.transaction_receipt().effective_gas_price(),
        //     order_executions,
        // })

        Ok(Default::default())
    }
}
