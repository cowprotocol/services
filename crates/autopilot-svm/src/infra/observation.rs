//! Settlement observation: tracks dispatched settlements as open windows in
//! `solana.settlement_executions` and resolves them against the
//! indexer-written `solana.settlements` rows.
//!
//! The executor opens a window per dispatched settlement. The indexer's
//! insert into `solana.settlements` fires the `solana_settlement_finalized`
//! NOTIFY (trigger in the schema), a [`ListenSession`] delivers it here, and
//! the window closes as `landed`. Windows whose submission deadline passes
//! without a settlement close as `timeout`. Windows live in the database, so
//! a restart mid-window loses nothing: the listen seed re-checks every open
//! window.

use {
    crate::infra::{db, listen::NotifyHandler},
    anyhow::{Context, Result},
    async_trait::async_trait,
    chain_types::solana::Pubkey,
    sqlx::PgPool,
};

/// Writes and closes settlement-execution windows.
///
/// `solana.settlement_executions.outcome` records what the indexer observed
/// on chain, never a driver's report, which is why a landing observed after
/// the deadline overwrites a timeout. The schema also allows `rejected`,
/// unused until typed `/settle` errors are consumed.
#[derive(Clone)]
pub struct SettlementTracker {
    pool: PgPool,
}

impl SettlementTracker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Open a window for a dispatched settlement. `solution_uid` is the
    /// winner's driver-local solution id until competition persistence
    /// allocates uids.
    pub async fn open_dispatched_settlement_window(
        &self,
        auction_id: i64,
        solver: Pubkey,
        solution_uid: u64,
        start_slot: u64,
        deadline_slot: u64,
    ) -> Result<()> {
        sqlx::query(
            r#"
INSERT INTO solana.settlement_executions
    (auction_id, solver, solution_uid, start_timestamp, start_slot, deadline_slot)
VALUES ($1, $2, $3, now(), $4, $5)
ON CONFLICT (auction_id, solver, solution_uid) DO NOTHING
            "#,
        )
        .bind(auction_id)
        .bind(solver.0)
        .bind(to_db_integer(solution_uid))
        .bind(to_db_integer(start_slot))
        .bind(to_db_integer(deadline_slot))
        .execute(&self.pool)
        .await
        .context("open settlement execution window")?;
        Ok(())
    }

    /// Close the solver's windows of the auction as landed, recording the
    /// settlement's slot and signature. Keyed by solver so concurrent winners
    /// of one auction each close on their own settlement. A window already
    /// closed as timed out upgrades to landed: the settlement executed, just
    /// late, and lateness stays visible as `end_slot` past `deadline_slot`.
    ///
    /// A settlement carries no solution uid, so a solver holding several
    /// windows of one auction closes all of them on its first settlement.
    /// Correct while one solver wins at most one solution per auction.
    async fn close_solvers_window_as_landed(
        &self,
        auction_id: i64,
        solver: &[u8],
        slot: i64,
        signature: &[u8],
    ) -> Result<u64> {
        let closed = sqlx::query(
            r#"
UPDATE solana.settlement_executions
SET outcome = 'landed', end_timestamp = now(), end_slot = $2, submitted_signature = $3
WHERE auction_id = $1 AND solver = $4 AND (outcome IS NULL OR outcome = 'timeout')
            "#,
        )
        .bind(auction_id)
        .bind(slot)
        .bind(signature)
        .bind(solver)
        .execute(&self.pool)
        .await
        .context("close settlement execution window")?
        .rows_affected();
        Ok(closed)
    }

    /// Close every open window whose deadline is at or before the slot as
    /// timed out, logging each. Driven by the competition cycle, so on a
    /// chain with no active auctions a timeout surfaces with the next
    /// competition, not at its deadline slot.
    pub async fn close_expired_windows_as_timeout(&self, slot: u64) -> Result<()> {
        let expired: Vec<(i64,)> = sqlx::query_as(
            r#"
UPDATE solana.settlement_executions
SET outcome = 'timeout', end_timestamp = now(), end_slot = $1
WHERE outcome IS NULL AND deadline_slot <= $1
RETURNING auction_id
            "#,
        )
        .bind(to_db_integer(slot))
        .fetch_all(&self.pool)
        .await
        .context("expire settlement execution windows")?;
        for (auction_id,) in expired {
            tracing::error!(auction_id, slot, "settlement missed its deadline");
        }
        Ok(())
    }

    /// Auction ids of open windows, the seed re-read set.
    async fn open_window_auction_ids(&self) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT auction_id FROM solana.settlement_executions WHERE outcome IS NULL",
        )
        .fetch_all(&self.pool)
        .await
        .context("read open settlement execution windows")?;
        Ok(rows.into_iter().map(|(auction_id,)| auction_id).collect())
    }
}

/// Slots and solution ids stay far below `i64::MAX`, the column type.
fn to_db_integer(value: u64) -> i64 {
    i64::try_from(value).expect("value exceeds i64")
}

/// Resolves NOTIFY payloads (and reconnect re-reads) against the settlements
/// table, closing the matching windows.
pub struct SettlementObservation {
    pool: PgPool,
    tracker: SettlementTracker,
}

impl SettlementObservation {
    pub fn new(pool: PgPool, tracker: SettlementTracker) -> Self {
        Self { pool, tracker }
    }

    /// Close the matching windows for every settlement observed for the
    /// auction, each keyed to its solver.
    async fn resolve(&self, auction_id: i64) -> Result<()> {
        for settlement in db::settlements_by_auction(&self.pool, auction_id).await? {
            let closed = self
                .tracker
                .close_solvers_window_as_landed(
                    auction_id,
                    &settlement.solver.0,
                    settlement.slot,
                    &settlement.tx_signature.0,
                )
                .await?;
            if closed > 0 {
                tracing::info!(
                    auction_id,
                    slot = settlement.slot,
                    tx_signature = %const_hex::encode(settlement.tx_signature.0),
                    "settlement observed on chain"
                );
            }
        }
        Ok(())
    }
}

#[async_trait]
impl NotifyHandler for SettlementObservation {
    /// Re-check every open window: a NOTIFY missed while the connection was
    /// down (or while the autopilot was not running) is recovered here.
    async fn seed(&mut self) -> Result<()> {
        for auction_id in self.tracker.open_window_auction_ids().await? {
            self.resolve(auction_id).await?;
        }
        Ok(())
    }

    async fn on_notify(&mut self, payload: &str) -> Result<()> {
        let Ok(auction_id) = payload.parse::<i64>() else {
            tracing::warn!(payload, "unparsable settlement notify payload");
            return Ok(());
        };
        self.resolve(auction_id).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{SettlementObservation, SettlementTracker},
        crate::infra::listen::ListenSession,
        chain_types::solana::Pubkey,
        sqlx::PgPool,
        std::time::Duration,
    };

    async fn wipe(pool: &PgPool) {
        for table in [
            "solana.trades",
            "solana.settlements",
            "solana.settlement_executions",
            "solana.order_events",
        ] {
            sqlx::query(&format!("DELETE FROM {table}"))
                .execute(pool)
                .await
                .unwrap();
        }
    }

    async fn insert_settlement(pool: &PgPool, auction_id: i64) {
        sqlx::query(
            r#"
INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, auction_id, solution_uid)
VALUES (10, $1, 0, $2, $3, NULL)
            "#,
        )
        .bind([9u8; 64])
        .bind([7u8; 32])
        .bind(auction_id)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn outcome(pool: &PgPool, auction_id: i64) -> Option<String> {
        sqlx::query_scalar("SELECT outcome FROM solana.settlement_executions WHERE auction_id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// The full path: a dispatched settlement opens a window, the trigger's
    /// NOTIFY (here: a bare INSERT, standing in for the indexer) closes it
    /// as landed with the settlement's signature.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_settlement_notify_closes_the_window_as_landed() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        wipe(&pool).await;

        let tracker = SettlementTracker::new(pool.clone());
        tracker
            .open_dispatched_settlement_window(4242, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();

        let task = ListenSession::spawn(
            pool.clone(),
            "solana_settlement_finalized",
            SettlementObservation::new(pool.clone(), tracker.clone()),
        );

        insert_settlement(&pool, 4242).await;

        for _ in 0..200 {
            if outcome(&pool, 4242).await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        task.abort();
        assert_eq!(outcome(&pool, 4242).await.as_deref(), Some("landed"));
        let signature: Vec<u8> = sqlx::query_scalar(
            "SELECT submitted_signature FROM solana.settlement_executions WHERE auction_id = 4242",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(signature, vec![9u8; 64]);
    }

    /// Expiry closes only windows past their deadline, and closed windows
    /// are not re-expired.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_expiry_times_out_only_past_deadlines() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        wipe(&pool).await;

        let tracker = SettlementTracker::new(pool.clone());
        tracker
            .open_dispatched_settlement_window(1, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();
        tracker
            .open_dispatched_settlement_window(2, Pubkey([7; 32]), 1, 90, 200)
            .await
            .unwrap();

        tracker.close_expired_windows_as_timeout(150).await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("timeout"));
        assert_eq!(outcome(&pool, 2).await, None);

        // A settlement observed after the timeout upgrades the verdict: it
        // executed, just late.
        tracker
            .close_solvers_window_as_landed(1, &[7u8; 32], 160, &[9u8; 64])
            .await
            .unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("landed"));
    }
}
