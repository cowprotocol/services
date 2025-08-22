use {
    sqlx::postgres::PgPool,
    std::time::{Duration, Instant},
};

struct PgLockGuard {
    conn: sqlx::pool::PoolConnection<sqlx::Postgres>,
    key: String,
}

impl PgLockGuard {
    async fn ping(&mut self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&mut *self.conn)
            .await
            .is_ok()
    }

    async fn unlock(mut self) {
        // TODO what can we do with the result?
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(self.key)
            .execute(&mut *self.conn)
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, "lock release failed");
            });
        // conn returns to pool unlocked
    }
}

pub struct LeaderLock {
    pool: PgPool,
    key: String,
    lock_guard: Option<PgLockGuard>,
    last_try: Instant,
    try_every: Duration,
}

impl LeaderLock {
    pub fn new(pool: PgPool, key: String, try_every: Duration) -> Self {
        Self {
            pool,
            key,
            lock_guard: None,
            last_try: Instant::now() - try_every, // allow immediate first try
            try_every,
        }
    }

    // Call every loop; handles acquire/release/liveness. Returns "am I leader rn?"
    pub async fn tick(&mut self) -> Result<bool, sqlx::Error> {
        // if we think we're leader, verify the session is alive
        if let Some(lock) = self.lock_guard.as_mut() {
            if !lock.ping().await {
                tracing::warn!("leader session died; demoting");
                self.lock_guard = None; // lock already gone with the dead session
            }
        }

        // try to become leader if we aren't (rate-limited)
        if self.lock_guard.is_none() && self.last_try.elapsed() >= self.try_every {
            self.last_try = Instant::now();
            let mut conn = self.pool.acquire().await?;
            let got_lock: bool =
                sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
                    .bind(&self.key)
                    .fetch_one(&mut *conn)
                    .await?;
            if got_lock {
                tracing::info!("became leader");
                self.lock_guard = Some(PgLockGuard {
                    conn,
                    key: self.key.to_owned(),
                });
            }
        }

        Ok(self.lock_guard.is_some())
    }

    // TODO: call on SIGTERM for graceful step-down
    pub async fn step_down(&mut self) {
        if let Some(lock) = self.lock_guard.take() {
            lock.unlock().await;
            tracing::info!("released leader lock");
        }
    }
}
