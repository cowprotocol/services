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
        const QUERY: &str = r#"SELECT 1"#;

        sqlx::query(QUERY).execute(&mut *self.conn).await.is_ok()
    }

    async fn unlock(mut self) {
        const QUERY: &str = r#"
SELECT pg_advisory_unlock(hashtextextended($1, 0));
        "#;

        let _ = sqlx::query(QUERY)
            .bind(self.key)
            .execute(&mut *self.conn)
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, "lock release failed");
            });
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

    /// Tries to acquire the leader lock and handles the liveness status.
    pub async fn try_acquire(&mut self) -> Result<bool, sqlx::Error> {
        const QUERY: &str = r#"
SELECT pg_try_advisory_lock(hashtextextended($1, 0));
        "#;

        // if we think we're leader, verify the session is alive
        if let Some(lock) = self.lock_guard.as_mut()
            && !lock.ping().await
        {
            tracing::warn!("leader session died");
            self.lock_guard = None; // lock already gone with the dead session
        }

        // try to become leader if we aren't (rate-limited)
        if self.lock_guard.is_none() && self.last_try.elapsed() >= self.try_every {
            self.last_try = Instant::now();
            let mut conn = self.pool.acquire().await?;
            let got_lock: bool = sqlx::query_scalar(QUERY)
                .bind(&self.key)
                .fetch_one(&mut *conn)
                .await?;
            if got_lock {
                tracing::info!("leader lock acquired");
                self.lock_guard = Some(PgLockGuard {
                    conn,
                    key: self.key.to_owned(),
                });
            }
        }

        Ok(self.lock_guard.is_some())
    }

    pub async fn release(&mut self) {
        if let Some(lock) = self.lock_guard.take() {
            lock.unlock().await;
            tracing::info!("released leader lock");
        }
    }
}
