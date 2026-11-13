//! Leader election between autopilot instances through a Postgres
//! session-level advisory lock.
//!
//! The leader pings its lock session every 2 s. Postgres closes a session
//! that runs no query for 10 s, which frees the lock, so a frozen leader
//! loses it. The leader stops acting 8 s after its last ping, before Postgres
//! can free the lock, and at once when a ping fails.

use {
    crate::LeaderLock,
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

/// How long Postgres keeps a silent lock session open. The other timings are
/// shares of it, so tests can shrink them all at once.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Holds the leader lock, or competes for it, from a background task until
/// released.
pub struct LeaderPgLock {
    state: Arc<State>,
    heartbeat: JoinHandle<()>,
}

#[derive(Default)]
struct State {
    /// Until when this instance may act as the leader.
    leader_until: Mutex<Option<Instant>>,
    /// Errors not yet returned by [`LeaderPgLock::take_errors`].
    errors: AtomicU64,
}

impl LeaderPgLock {
    pub fn spawn(pool: &PgPool, key: String) -> Self {
        Self::spawn_with_idle_timeout(pool, key, IDLE_TIMEOUT)
    }

    fn spawn_with_idle_timeout(pool: &PgPool, key: String, idle_timeout: Duration) -> Self {
        let state = Arc::new(State::default());
        let mut session = Session::new(pool, key, idle_timeout);
        let heartbeat = tokio::spawn({
            let state = state.clone();
            async move {
                loop {
                    let held_lock = session.holds_lock;
                    let (leader_until, wait) = match session.beat().await {
                        Ok(Some(leader_until)) => (Some(leader_until), session.heartbeat()),
                        Ok(None) => (None, session.follower_poll()),
                        Err(err) => {
                            tracing::error!(?err, held_lock, "leader lock heartbeat failed");
                            state.errors.fetch_add(1, Ordering::Relaxed);
                            (None, session.heartbeat())
                        }
                    };
                    *state.leader_until.lock().unwrap() = leader_until;
                    tokio::time::sleep(wait).await;
                }
            }
        });
        Self { state, heartbeat }
    }

    /// Whether this instance may act as the leader right now. The lock can
    /// be lost at any time, so check it right before every leader-only action.
    pub fn is_leader(&self) -> bool {
        self.state
            .leader_until
            .lock()
            .unwrap()
            .is_some_and(|end| Instant::now() < end)
    }

    /// Returns how many heartbeats failed since the last call, and resets the
    /// count.
    pub fn take_errors(&self) -> u64 {
        self.state.errors.swap(0, Ordering::Relaxed)
    }

    /// Stops the heartbeat and closes its session, which frees the lock.
    pub async fn release(mut self) {
        self.heartbeat.abort();
        let _ = (&mut self.heartbeat).await;
    }
}

#[async_trait::async_trait]
impl LeaderLock for LeaderPgLock {
    fn is_leader(&self) -> bool {
        LeaderPgLock::is_leader(self)
    }

    fn take_errors(&self) -> u64 {
        LeaderPgLock::take_errors(self)
    }

    async fn release(self: Box<Self>) {
        LeaderPgLock::release(*self).await
    }
}

impl Drop for LeaderPgLock {
    fn drop(&mut self) {
        // after `release` the task has finished, and `abort` on a finished
        // task does nothing
        self.heartbeat.abort();
    }
}

/// The connection that holds the lock, or tries to take it.
struct Session {
    options: PgConnectOptions,
    key: String,
    idle_timeout: Duration,
    conn: Option<PgConnection>,
    holds_lock: bool,
}

impl Session {
    fn new(pool: &PgPool, key: String, idle_timeout: Duration) -> Self {
        Self {
            // Postgres closes a session that stays silent for the timeout,
            // which frees the lock even while the leader's TCP connection
            // stays up.
            options: (*pool.connect_options())
                .clone()
                .options([("idle_session_timeout", idle_timeout.as_millis())]),
            key,
            idle_timeout,
            conn: None,
            holds_lock: false,
        }
    }

    /// How often the leader pings, and how long anyone waits after a failed
    /// query: 2 s.
    fn heartbeat(&self) -> Duration {
        self.idle_timeout / 5
    }

    /// How often a follower tries to take the lock: 200 ms, so it takes over
    /// right after the lock is freed.
    fn follower_poll(&self) -> Duration {
        self.idle_timeout / 50
    }

    /// How long a query may take before it fails: 5 s. 2 s between pings plus
    /// 5 s stays under the leader's 8 s, so a stuck ping fails first.
    fn query_timeout(&self) -> Duration {
        self.idle_timeout / 2
    }

    /// How long the leader acts after sending a ping: 8 s. Postgres closes the
    /// session 10 s after it answered, which is later, so the leader stops
    /// first.
    fn leader_for(&self) -> Duration {
        self.idle_timeout - self.heartbeat()
    }

    /// Pings the session while it holds the lock, and tries to take the lock
    /// otherwise. Returns until when this instance may act as the leader.
    async fn beat(&mut self) -> Result<Option<Instant>, sqlx::Error> {
        let sent = Instant::now();
        let result = tokio::time::timeout(self.query_timeout(), self.query())
            .await
            .unwrap_or_else(|_| Err(io::Error::from(io::ErrorKind::TimedOut).into()));
        if result.is_err() {
            // a failed or timed-out query drops its connection and the lock
            self.holds_lock = false;
        }
        result?;
        Ok(self.holds_lock.then(|| sent + self.leader_for()))
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

    const IDLE_TIMEOUT: Duration = Duration::from_secs(2);

    /// A leader that goes silent, like one on a frozen node, loses the lock
    /// once Postgres closes its session, and stops acting before another
    /// instance can take over.
    #[tokio::test]
    #[ignore]
    async fn postgres_silent_leader_loses_the_lock() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let key = "postgres_silent_leader_loses_the_lock".to_string();
        let mut leader = Session::new(&pool, key.clone(), IDLE_TIMEOUT);
        let mut follower = Session::new(&pool, key, IDLE_TIMEOUT);

        // Heartbeats keep the lock with the leader for longer than the timeout.
        let mut leader_until = None;
        for _ in 0..10 {
            leader_until = leader.beat().await.unwrap();
            assert!(leader_until.is_some());
            assert_eq!(follower.beat().await.unwrap(), None);
            tokio::time::sleep(leader.heartbeat()).await;
        }

        let silent_since = Instant::now();
        let took_over = loop {
            if follower.beat().await.unwrap().is_some() {
                break Instant::now();
            }
            assert!(
                silent_since.elapsed() < 2 * IDLE_TIMEOUT,
                "the follower never took over"
            );
            tokio::time::sleep(follower.follower_poll()).await;
        };
        assert!(leader_until.unwrap() < took_over);

        // The old leader finds its session closed.
        assert!(leader.beat().await.is_err());
    }

    /// A released lock passes to the follower at its next heartbeat.
    #[tokio::test]
    #[ignore]
    async fn postgres_released_lock_passes_to_the_follower() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let key = "postgres_released_lock_passes_to_the_follower".to_string();
        let leader = LeaderPgLock::spawn_with_idle_timeout(&pool, key.clone(), IDLE_TIMEOUT);
        wait_until(|| leader.is_leader()).await;

        let follower = LeaderPgLock::spawn_with_idle_timeout(&pool, key, IDLE_TIMEOUT);
        tokio::time::sleep(IDLE_TIMEOUT).await;
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
        tokio::time::timeout(IDLE_TIMEOUT, wait)
            .await
            .expect("condition not met within the timeout");
    }
}
