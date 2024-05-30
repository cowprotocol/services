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
        database::Postgres,
        decoded_settlement::DecodedSettlement,
        domain::{eth, settlement::Transaction},
        infra,
    },
    anyhow::{Context, Result},
    primitive_types::H256,
    sqlx::PgConnection,
    std::{collections::BTreeMap, sync::Arc},
    tokio::sync::{Mutex, Notify},
};

pub struct OnSettlementEvent {
    inner: Arc<Inner>,
}

struct Inner {
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

pub struct CircuitBreaker {
    circuit_breaker: infra::blockchain::circuit_breaker::CircuitBreaker,
    db: Postgres,
    solvers: Vec<eth::Address>,
    // Registry to keep track of the already processed settlements, format (block, hash)
    registry: Arc<Mutex<BTreeMap<i64, H256>>>,
}

impl CircuitBreaker {
    /// Maximum capacity of the registry in order not to overflow the memory
    const MAX_REGISTRY_SIZE: usize = 100;
    /// The number of settlements taken from the database (ordered by block)
    const NUMBER_SETTLEMENTS: i64 = 4;

    pub fn build(
        circuit_breaker: infra::blockchain::circuit_breaker::CircuitBreaker,
        solvers: Vec<eth::Address>,
        db: Postgres,
    ) -> Self {
        Self {
            circuit_breaker,
            db,
            solvers,
            registry: Arc::new(Mutex::new(BTreeMap::new())),
        }
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
            (Some(score), _) if score.winner.0 != tx_from.0 => {
                tracing::warn!(
                    auction_id,
                    ?tx_from,
                    winner = ?score.winner,
                    "solution submitted by solver other than the winner"
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
            (Some(_), false) => Ok(AuctionIdRecoveryStatus::AddAuctionData(
                auction_id, settlement,
            )),
        }
    }

    async fn apply(&self) -> Result<bool> {
        let mut ex = self
            .db
            .pool
            .begin()
            .await
            .context("acquire DB connection")?;
        let mut events = database::settlements::get_settlements_without_auction(
            &mut ex,
            Self::NUMBER_SETTLEMENTS,
        )
        .await
        .context("get_settlement_without_auction")?
        .into_iter()
        .map(|event| (event.block_number, H256(event.tx_hash.0)))
        .collect::<BTreeMap<_, _>>();

        {
            let mut registry = self.registry.lock().await;

            let new_events = events
                .iter()
                .filter(|&(k, _)| !registry.contains_key(k))
                .map(|(&k, &v)| (k, v))
                .collect::<BTreeMap<_, _>>();

            if new_events.is_empty() {
                return Ok(false);
            }

            // @TODO: Write here the check for the settlement validity

            // We want to keep the registry with a max size of MAX_REGISTRY_SIZE, since the
            // BTreeMap is ordered by key, every time there is a buffer
            // overflow, the oldest blocks are removed This way we always
            // preserve the most up-to-date data in-memory
            registry.append(&mut events);
            if registry.len() > Self::MAX_REGISTRY_SIZE {
                (0..(registry.len() - Self::MAX_REGISTRY_SIZE)).for_each(|_| {
                    registry.pop_first();
                })
            }
        }

        Ok(true)
    }
}
