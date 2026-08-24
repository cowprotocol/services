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
    anyhow::Result,
    async_trait::async_trait,
    chain_types::solana::Pubkey,
    sqlx::PgPool,
};

/// Opens and expires settlement-execution windows.
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
        db::open_settlement_window(
            &self.pool,
            auction_id,
            solver,
            to_db_integer(solution_uid),
            to_db_integer(start_slot),
            to_db_integer(deadline_slot),
        )
        .await
    }

    /// Close every open window whose deadline is at or before the slot as
    /// timed out, logging each. Driven by the competition cycle, so on a
    /// chain with no active auctions a timeout surfaces with the next
    /// competition, not at its deadline slot.
    pub async fn close_expired_windows_as_timeout(&self, slot: u64) -> Result<()> {
        let slot = to_db_integer(slot);
        for auction_id in db::expire_settlement_windows(&self.pool, slot).await? {
            tracing::error!(auction_id, slot, "settlement missed its deadline");
        }
        Ok(())
    }
}

/// Slots and solution ids stay far below `i64::MAX`, the column type.
fn to_db_integer(value: u64) -> i64 {
    i64::try_from(value).expect("value exceeds i64")
}

/// Closes windows against the settlements the indexer records, driven by the
/// `solana_settlement_finalized` notifications.
pub struct SettlementObservation {
    pool: PgPool,
}

impl SettlementObservation {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Close the auction's windows against its observed settlements.
    async fn resolve(&self, auction_id: i64) -> Result<()> {
        for landed in db::close_landed_windows(&self.pool, auction_id).await? {
            tracing::info!(
                auction_id,
                slot = landed.end_slot,
                solver = %Pubkey(landed.solver.0),
                tx_signature = %const_hex::encode(landed.submitted_signature.0),
                "settlement observed on chain"
            );
        }
        Ok(())
    }
}

#[async_trait]
impl NotifyHandler for SettlementObservation {
    /// Re-check every open window: a NOTIFY missed while the connection was
    /// down (or while the autopilot was not running) is recovered here.
    async fn seed(&mut self) -> Result<()> {
        for auction_id in db::open_window_auction_ids(&self.pool).await? {
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
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let tracker = SettlementTracker::new(pool.clone());
        tracker
            .open_dispatched_settlement_window(4242, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();

        let task = ListenSession::spawn(
            pool.clone(),
            "solana_settlement_finalized",
            SettlementObservation::new(pool.clone()),
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
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

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
        insert_settlement(&pool, 1).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("landed"));
    }
}
