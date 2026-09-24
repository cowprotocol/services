//! Settlement windows: dispatched settlements tracked in
//! `solana.settlement_executions` and resolved against the indexer-written
//! `solana.settlements` rows.
//!
//! The executor opens a window per dispatched settlement. The indexer's
//! insert into `solana.settlements` fires the `solana_settlement_finalized`
//! NOTIFY (trigger in the schema), a [`ListenSession`] delivers it here, and
//! the window closes as `landed`. The task that dispatched the settlement
//! waits out the submission deadline and closes the window as `timeout` if
//! nothing landed by then. A landing is only visible once the indexer wrote
//! it, so deadlines are judged against the indexer's watermark, not the raw
//! tip, until the indexer lags beyond its allowance. Windows live in the
//! database, so a restart mid-window loses nothing: the listen seed re-checks
//! every open window and the startup sweep times out the overdue ones.

use {
    crate::infra::{db, listen::NotifyHandler},
    anyhow::Result,
    async_trait::async_trait,
    chain_types::solana::{Pubkey, Signature},
    sqlx::PgPool,
    tokio::sync::watch,
};

/// The settlement-execution windows in `solana.settlement_executions`: the
/// executor opens one per dispatched settlement and times it out at its
/// deadline, and the `solana_settlement_finalized` notifications close the
/// ones the indexer saw land.
///
/// `outcome` records what the indexer observed on chain, which is why a
/// landing observed after the deadline overwrites a timeout. The one
/// exception is `rejected`, written from a driver error that proves no
/// transaction went out, leaving nothing for the indexer to observe.
#[derive(Clone)]
pub struct SettlementWindows {
    pool: PgPool,
    /// The chain tip, advanced by the slot poller.
    tip: watch::Receiver<u64>,
    /// Slots the indexer may trail the tip before its silence about a
    /// settlement stops counting as evidence.
    max_indexer_lag: u64,
}

impl SettlementWindows {
    pub fn new(pool: PgPool, tip: watch::Receiver<u64>, max_indexer_lag: u64) -> Self {
        Self {
            pool,
            tip,
            max_indexer_lag,
        }
    }

    /// The slot up to which a settlement can be ruled out: the indexer's
    /// watermark, or the tip itself once the indexer lags beyond its
    /// allowance or never wrote.
    async fn observed_slot(&self, tip: u64) -> Result<u64> {
        let indexed = db::last_indexed_slot(&self.pool)
            .await?
            .and_then(|slot| u64::try_from(slot).ok());
        Ok(match indexed {
            Some(indexed) if tip.saturating_sub(indexed) <= self.max_indexer_lag => {
                indexed.min(tip)
            }
            _ => tip,
        })
    }

    /// Open a window for a dispatched settlement. `solution_uid` is the
    /// autopilot-generated uid the competition persisted.
    pub async fn open_dispatched(
        &self,
        auction_id: i64,
        solver: Pubkey,
        solution_uid: i64,
        start_slot: u64,
        deadline_slot: u64,
    ) -> Result<()> {
        db::open_settlement_window(
            &self.pool,
            auction_id,
            solver,
            solution_uid,
            to_db_integer(start_slot),
            to_db_integer(deadline_slot),
        )
        .await
    }

    /// Close the window of a settlement that provably never left the driver,
    /// so it does not sit open until the deadline sweep mislabels it a
    /// timeout. An ambiguous failure keeps its window: the transaction may
    /// still land, and only an open window is re-checked against the indexer.
    pub async fn close_rejected(
        &self,
        auction_id: i64,
        solver: Pubkey,
        solution_uid: i64,
    ) -> Result<()> {
        db::reject_settlement_window(&self.pool, auction_id, solver, solution_uid).await
    }

    /// Close every open window whose deadline the indexer has processed
    /// without a settlement as timed out, logging each. Runs at startup for
    /// the windows a previous process left behind and inside every
    /// competition cycle. A live dispatch times its own window out through
    /// [`Self::expire_when_due`].
    pub async fn expire_past_deadline(&self, tip: u64) -> Result<()> {
        let slot = to_db_integer(self.observed_slot(tip).await?);
        for auction_id in db::expire_settlement_windows(&self.pool, slot).await? {
            tracing::error!(auction_id, slot, "settlement missed its deadline");
        }
        Ok(())
    }

    /// Wait for the tip to reach the window's deadline and for the indexer to
    /// have processed it, then close the window as timed out if the indexer
    /// has not closed it by then. A database error is retried on the next
    /// slot: giving up would leave the window to a cycle sweep, and an idle
    /// chain runs none. Returns once the window is resolved either way, or
    /// when the slot poller is gone.
    pub async fn expire_when_due(
        &self,
        auction_id: i64,
        solver: Pubkey,
        solution_uid: i64,
        deadline_slot: u64,
    ) {
        let mut tip = self.tip.clone();
        let mut current = match tip.wait_for(|slot| *slot >= deadline_slot).await {
            Ok(slot) => *slot,
            Err(_) => return,
        };
        loop {
            match self
                .expire_once_processed(auction_id, solver, solution_uid, deadline_slot, current)
                .await
            {
                Ok(true) => return,
                Ok(false) => {}
                Err(err) => tracing::warn!(
                    auction_id,
                    solution_uid,
                    ?err,
                    "failed to time out the settlement window, retrying next slot"
                ),
            }
            current = match tip.wait_for(|slot| *slot > current).await {
                Ok(slot) => *slot,
                Err(_) => return,
            };
        }
    }

    /// Time the window out once the indexer has processed its deadline at
    /// the tip. Answers whether the window is resolved: timed out here, or
    /// closed before.
    async fn expire_once_processed(
        &self,
        auction_id: i64,
        solver: Pubkey,
        solution_uid: i64,
        deadline_slot: u64,
        tip: u64,
    ) -> Result<bool> {
        let observed = self.observed_slot(tip).await?;
        if observed < deadline_slot {
            return Ok(false);
        }
        let slot = to_db_integer(observed);
        let expired =
            db::expire_settlement_window(&self.pool, auction_id, solver, solution_uid, slot)
                .await?;
        if expired {
            tracing::error!(auction_id, %solver, solution_uid, slot, "settlement missed its deadline");
        }
        Ok(true)
    }

    /// Close the auction's windows against its observed settlements.
    async fn close_landed(&self, auction_id: i64) -> Result<()> {
        for landed in db::close_landed_windows(&self.pool, auction_id).await? {
            let solver = Pubkey(landed.solver.0);
            tracing::info!(
                auction_id,
                slot = landed.end_slot,
                %solver,
                tx_signature = %Signature(landed.submitted_signature.0),
                "settlement observed on chain"
            );
        }
        Ok(())
    }
}

/// Slots and solution ids stay far below `i64::MAX`, the column type.
fn to_db_integer(value: u64) -> i64 {
    i64::try_from(value).expect("value exceeds i64")
}

#[async_trait]
impl NotifyHandler for SettlementWindows {
    /// Re-check every open window: a NOTIFY missed while the connection was
    /// down (or while the autopilot was not running) is recovered here.
    async fn seed(&mut self) -> Result<()> {
        for auction_id in db::open_window_auction_ids(&self.pool).await? {
            self.close_landed(auction_id).await?;
        }
        Ok(())
    }

    async fn on_notify(&mut self, payload: &str) -> Result<()> {
        let Ok(auction_id) = payload.parse::<i64>() else {
            tracing::warn!(payload, "unparsable settlement notify payload");
            return Ok(());
        };
        self.close_landed(auction_id).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::SettlementWindows,
        crate::infra::{db, listen::ListenSession},
        chain_types::solana::Pubkey,
        sqlx::{PgPool, postgres::PgPoolOptions},
        std::time::Duration,
        tokio::sync::watch,
    };

    /// Windows over a tip that never moves.
    fn windows(pool: &PgPool) -> SettlementWindows {
        SettlementWindows::new(pool.clone(), watch::channel(0).1, 150)
    }

    async fn set_indexed_slot(pool: &PgPool, slot: i64) {
        sqlx::query("DELETE FROM solana.indexer_state")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO solana.indexer_state (slot) VALUES ($1)")
            .bind(slot)
            .execute(pool)
            .await
            .unwrap();
    }

    /// A persisted execution of order `[order; 32]` inside solution `uid` of
    /// the auction.
    async fn insert_execution(pool: &PgPool, auction_id: i64, uid: i64, order: u8) {
        sqlx::query(
            "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, order_uid, \
             executed_sell, executed_buy) VALUES ($1, $2, $3, 10, 20)",
        )
        .bind(auction_id)
        .bind(uid)
        .bind([order; 32])
        .execute(pool)
        .await
        .unwrap();
    }

    /// A landed settlement of the auction by solver 7 under signature
    /// `[signature; 64]`, trading the given orders. The trades go in first:
    /// the indexer commits both together, so the NOTIFY the settlement fires
    /// never sees a settlement without its trades.
    async fn insert_settlement(pool: &PgPool, auction_id: i64, signature: u8, orders: &[u8]) {
        for order in orders {
            sqlx::query(
                "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, \
                 sell_amount, buy_amount, fee_amount) VALUES ($1, 0, $2, 10, 20, 0)",
            )
            .bind([signature; 64])
            .bind([*order; 32])
            .execute(pool)
            .await
            .unwrap();
        }
        sqlx::query(
            r#"
INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, auction_id, solution_uid)
VALUES (10, $1, 0, $2, $3, NULL)
            "#,
        )
        .bind([signature; 64])
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

    /// Outcome and signature of every window of the auction, by solution uid.
    async fn windows_of(
        pool: &PgPool,
        auction_id: i64,
    ) -> Vec<(i64, Option<String>, Option<Vec<u8>>)> {
        sqlx::query_as(
            "SELECT solution_uid, outcome, submitted_signature FROM solana.settlement_executions \
             WHERE auction_id = $1 ORDER BY solution_uid",
        )
        .bind(auction_id)
        .fetch_all(pool)
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

        let solver = Pubkey([7; 32]);
        let windows = windows(&pool);
        windows
            .open_dispatched(4242, solver, 1, 90, 100)
            .await
            .unwrap();
        insert_execution(&pool, 4242, 1, 1).await;

        let task = ListenSession::spawn(
            pool.clone(),
            db::SETTLEMENT_FINALIZED_CHANNEL,
            windows.clone(),
        );

        insert_settlement(&pool, 4242, 9, &[1]).await;

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

        let windows = windows(&pool);
        windows
            .open_dispatched(1, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();
        windows
            .open_dispatched(2, Pubkey([7; 32]), 1, 90, 200)
            .await
            .unwrap();

        windows.expire_past_deadline(150).await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("timeout"));
        assert_eq!(outcome(&pool, 2).await, None);

        // A settlement observed after the timeout upgrades the verdict: it
        // executed, just late.
        insert_execution(&pool, 1, 1, 1).await;
        insert_settlement(&pool, 1, 9, &[1]).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("landed"));
    }

    /// A rejected window is closed before its deadline, so the expiry sweep
    /// leaves it alone instead of relabelling it a timeout.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_rejection_closes_the_window_before_the_deadline() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let windows = windows(&pool);
        windows
            .open_dispatched(1, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();

        windows.close_rejected(1, Pubkey([7; 32]), 1).await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("rejected"));

        windows.expire_past_deadline(150).await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("rejected"));
    }

    /// An observed settlement outranks a driver's rejection: a rejection
    /// arriving after the window landed leaves the verdict alone.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_rejection_does_not_overwrite_a_landed_window() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let windows = windows(&pool);
        windows
            .open_dispatched(1, Pubkey([7; 32]), 1, 90, 100)
            .await
            .unwrap();

        insert_execution(&pool, 1, 1, 1).await;
        insert_settlement(&pool, 1, 9, &[1]).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("landed"));

        windows.close_rejected(1, Pubkey([7; 32]), 1).await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("landed"));
    }

    /// A solver holding two windows of one auction: the settlement that
    /// traded the first solution's order closes only that window, the second
    /// window closes on its own settlement with its own signature.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_a_landing_closes_only_the_window_it_executed() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let solver = Pubkey([7; 32]);
        let windows = windows(&pool);
        for (uid, order) in [(1, 1u8), (2, 2)] {
            windows
                .open_dispatched(1, solver, uid, 90, 100)
                .await
                .unwrap();
            insert_execution(&pool, 1, uid, order).await;
        }

        insert_settlement(&pool, 1, 9, &[1]).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        assert_eq!(
            windows_of(&pool, 1).await,
            vec![
                (1, Some("landed".to_string()), Some(vec![9u8; 64])),
                (2, None, None),
            ]
        );

        insert_settlement(&pool, 1, 8, &[2]).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        assert_eq!(
            windows_of(&pool, 1).await,
            vec![
                (1, Some("landed".to_string()), Some(vec![9u8; 64])),
                (2, Some("landed".to_string()), Some(vec![8u8; 64])),
            ]
        );
    }

    /// The dispatching task waits for the tip to reach the deadline and for
    /// the indexer to have processed it, then times out the window still open
    /// and leaves the one the indexer closed.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_dispatch_task_times_out_its_window_at_the_deadline() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let solver = Pubkey([7; 32]);
        let (tip, receiver) = watch::channel(90);
        let windows = SettlementWindows::new(pool.clone(), receiver, 150);
        for uid in [1, 2] {
            windows
                .open_dispatched(1, solver, uid, 90, 100)
                .await
                .unwrap();
        }
        insert_execution(&pool, 1, 2, 2).await;
        insert_settlement(&pool, 1, 9, &[2]).await;
        crate::infra::db::close_landed_windows(&pool, 1)
            .await
            .unwrap();
        set_indexed_slot(&pool, 95).await;

        let waiters = tokio::spawn({
            let windows = windows.clone();
            async move {
                windows.expire_when_due(1, solver, 1, 100).await;
                windows.expire_when_due(1, solver, 2, 100).await;
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !waiters.is_finished(),
            "nothing expires before the deadline"
        );

        // The tip reached the deadline but the indexer has not: a landing at
        // the deadline could still be unwritten.
        tip.send(100).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !waiters.is_finished(),
            "nothing expires before the indexer processed the deadline"
        );

        set_indexed_slot(&pool, 100).await;
        tip.send(101).unwrap();
        waiters.await.unwrap();
        assert_eq!(
            windows_of(&pool, 1).await,
            vec![
                (1, Some("timeout".to_string()), None),
                (2, Some("landed".to_string()), Some(vec![9u8; 64])),
            ]
        );
    }

    /// An indexer stalled beyond its allowance stops gating: the window times
    /// out on the tip alone.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_dispatch_task_times_out_past_the_indexer_lag_allowance() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        let solver = Pubkey([7; 32]);
        let (tip, receiver) = watch::channel(90);
        let windows = SettlementWindows::new(pool.clone(), receiver, 150);
        windows
            .open_dispatched(1, solver, 1, 90, 100)
            .await
            .unwrap();
        set_indexed_slot(&pool, 95).await;

        let waiter = tokio::spawn({
            let windows = windows.clone();
            async move { windows.expire_when_due(1, solver, 1, 100).await }
        });
        // Indexed 95, allowance 150: the indexer still counts up to tip 245.
        tip.send(245).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!waiter.is_finished(), "the indexer is within its allowance");

        tip.send(246).unwrap();
        waiter.await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("timeout"));
    }

    /// A database error at the deadline does not drop the waiter: the next
    /// slot retries and times the window out.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_dispatch_task_retries_a_failed_timeout_on_the_next_slot() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        // A single-connection pool: holding its connection makes every
        // query of the waiter fail until it is released.
        let starved = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(50))
            .connect("postgresql://")
            .await
            .unwrap();
        let held = starved.acquire().await.unwrap();

        let solver = Pubkey([7; 32]);
        let (tip, receiver) = watch::channel(90);
        let windows = SettlementWindows::new(starved.clone(), receiver, 150);
        crate::infra::db::open_settlement_window(&pool, 1, solver, 1, 90, 100)
            .await
            .unwrap();
        set_indexed_slot(&pool, 100).await;

        let waiter = tokio::spawn({
            let windows = windows.clone();
            async move { windows.expire_when_due(1, solver, 1, 100).await }
        });
        tip.send(100).unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !waiter.is_finished(),
            "the failed attempt waits for the next slot"
        );
        assert_eq!(outcome(&pool, 1).await, None);

        drop(held);
        tip.send(101).unwrap();
        waiter.await.unwrap();
        assert_eq!(outcome(&pool, 1).await.as_deref(), Some("timeout"));
    }
}
