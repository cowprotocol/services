//! Leader election between autopilot instances through a Postgres
//! session-level advisory lock.

use {
    sqlx::{
        Connection,
        PgConnection,
        postgres::{PgConnectOptions, PgPool},
    },
    std::{
        io,
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    },
    tokio::task::JoinHandle,
};

/// How long Postgres keeps a silent lock session open. A leader beats every
/// fifth of it and stops acting a fifth of it before the lease ends. A beat
/// may take half of it, so a stuck beat fails before the lease ends. A
/// follower polls every fiftieth of it.
const LEASE: Duration = Duration::from_secs(10);

/// Holds the leader lock, or competes for it, from a background task until
/// released.
pub struct LeaderLock {
    state: Arc<State>,
    heartbeat: JoinHandle<()>,
}

#[derive(Default)]
struct State {
    /// Until when this instance may act as the leader.
    lease_end: Mutex<Option<Instant>>,
    /// Errors not yet returned by [`LeaderLock::take_errors`].
    errors: AtomicU64,
}

impl LeaderLock {
    pub fn spawn(pool: &PgPool, key: String) -> Self {
        Self::spawn_with_lease(pool, key, LEASE)
    }

    fn spawn_with_lease(pool: &PgPool, key: String, lease: Duration) -> Self {
        let state = Arc::new(State::default());
        let mut session = Session::new(pool, key, lease);
        let heartbeat = tokio::spawn({
            let state = state.clone();
            async move {
                loop {
                    let (lease_end, wait) = match session.beat().await {
                        Ok(Some(lease_end)) => (Some(lease_end), lease / 5),
                        // another instance leads: poll often to take over fast
                        Ok(None) => (None, lease / 50),
                        Err(err) => {
                            tracing::error!(?err, "leader lock heartbeat failed");
                            state.errors.fetch_add(1, Ordering::Relaxed);
                            (None, lease / 5)
                        }
                    };
                    *state.lease_end.lock().unwrap() = lease_end;
                    tokio::time::sleep(wait).await;
                }
            }
        });
        Self { state, heartbeat }
    }

    /// Whether this instance may act as the leader right now. The lease can
    /// end at any time, so check it right before every leader-only action.
    pub fn is_leader(&self) -> bool {
        // a panicked heartbeat dropped its session, and the lock with it
        !self.heartbeat.is_finished()
            && self
                .state
                .lease_end
                .lock()
                .unwrap()
                .is_some_and(|end| Instant::now() < end)
    }

    /// Returns how often taking or keeping the lock failed since the last
    /// call.
    pub fn take_errors(&self) -> u64 {
        self.state.errors.swap(0, Ordering::Relaxed)
    }

    /// Stops the heartbeat and closes its session, which frees the lock.
    pub async fn release(mut self) {
        self.heartbeat.abort();
        if let Err(err) = (&mut self.heartbeat).await
            && err.is_panic()
        {
            tracing::error!(?err, "leader lock heartbeat panicked");
        }
    }
}

impl Drop for LeaderLock {
    fn drop(&mut self) {
        self.heartbeat.abort();
    }
}

/// The connection that holds the lock, or tries to take it.
struct Session {
    options: PgConnectOptions,
    key: String,
    lease: Duration,
    conn: Option<PgConnection>,
    holds_lock: bool,
}

impl Session {
    fn new(pool: &PgPool, key: String, lease: Duration) -> Self {
        Self {
            // Postgres closes a session that stays silent for a lease, which
            // frees the lock even while the leader's TCP connection stays up.
            options: (*pool.connect_options())
                .clone()
                .options([("idle_session_timeout", lease.as_millis())]),
            key,
            lease,
            conn: None,
            holds_lock: false,
        }
    }

    /// Pings the session while it holds the lock, and tries to take the lock
    /// otherwise. Returns until when this instance may act as the leader.
    async fn beat(&mut self) -> Result<Option<Instant>, sqlx::Error> {
        let sent = Instant::now();
        let result = tokio::time::timeout(self.lease / 2, self.query())
            .await
            .unwrap_or_else(|_| Err(io::Error::from(io::ErrorKind::TimedOut).into()));
        if result.is_err() {
            // a failed or timed-out query drops its connection and the lock
            self.holds_lock = false;
        }
        result?;
        // Postgres closes the session a lease after it answered, which is
        // after `sent`, so the leader stops acting first.
        Ok(self.holds_lock.then(|| sent + self.lease - self.lease / 5))
    }

    async fn query(&mut self) -> Result<(), sqlx::Error> {
        let mut conn = match self.conn.take() {
            Some(conn) => conn,
            None => PgConnection::connect_with(&self.options).await?,
        };
        if self.holds_lock {
            sqlx::query("SELECT 1").execute(&mut conn).await?;
        } else {
            self.holds_lock =
                sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
                    .bind(&self.key)
                    .fetch_one(&mut conn)
                    .await?;
        }
        self.conn = Some(conn);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEASE: Duration = Duration::from_secs(2);

    /// A leader that goes silent, like one on a frozen node, loses the lock
    /// once Postgres closes its session, and its own lease ends before
    /// another instance can take over.
    #[tokio::test]
    #[ignore]
    async fn postgres_silent_leader_loses_the_lock() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let key = "postgres_silent_leader_loses_the_lock".to_string();
        let mut leader = Session::new(&pool, key.clone(), LEASE);
        let mut follower = Session::new(&pool, key, LEASE);

        // Heartbeats keep the lock with the leader for longer than a lease.
        let mut lease_end = None;
        for _ in 0..10 {
            lease_end = leader.beat().await.unwrap();
            assert!(lease_end.is_some());
            assert_eq!(follower.beat().await.unwrap(), None);
            tokio::time::sleep(LEASE / 5).await;
        }

        let silent_since = Instant::now();
        let took_over = loop {
            if follower.beat().await.unwrap().is_some() {
                break Instant::now();
            }
            assert!(
                silent_since.elapsed() < 2 * LEASE,
                "the follower never took over"
            );
            tokio::time::sleep(LEASE / 5).await;
        };
        assert!(lease_end.unwrap() < took_over);

        // The old leader finds its session closed.
        assert!(leader.beat().await.is_err());
    }

    /// A released lock passes to the follower at its next heartbeat.
    #[tokio::test]
    #[ignore]
    async fn postgres_released_lock_passes_to_the_follower() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let key = "postgres_released_lock_passes_to_the_follower".to_string();
        let leader = LeaderLock::spawn_with_lease(&pool, key.clone(), LEASE);
        wait_until(|| leader.is_leader()).await;

        let follower = LeaderLock::spawn_with_lease(&pool, key, LEASE);
        tokio::time::sleep(LEASE).await;
        assert!(!follower.is_leader());

        leader.release().await;
        wait_until(|| follower.is_leader()).await;
    }

    async fn wait_until(condition: impl Fn() -> bool) {
        let wait = async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::time::timeout(LEASE, wait)
            .await
            .expect("condition not met within a lease");
    }
}
