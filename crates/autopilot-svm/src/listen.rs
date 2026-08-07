//! `ListenSession`: a Postgres `LISTEN` wrapper that stays convergent across
//! reconnects.
//!
//! The ordering guarantee: subscribe, then seed a full read, then drain
//! notifications. A NOTIFY that fires between the subscribe and the seed is
//! buffered by the listener, not lost. On reconnect the seed re-runs, so a
//! NOTIFY missed while the connection was down is recovered by the next full
//! read.

use {
    anyhow::{Context, Result},
    async_trait::async_trait,
    sqlx::postgres::{PgListener, PgPool},
    std::{convert::Infallible, time::Duration},
};

/// What a [`ListenSession`] drives: a full re-read and a per-notification
/// handler. Each cache the autopilot keeps convergent implements one.
#[async_trait]
pub trait NotifyHandler: Send {
    /// Full re-read of the backing table. Runs once the `LISTEN` is active and
    /// again after every reconnect.
    async fn seed(&mut self) -> Result<()>;

    /// Handle one NOTIFY payload.
    async fn on_notify(&mut self, payload: &str) -> Result<()>;
}

/// Drives a [`NotifyHandler`] against one channel, reconnecting with capped
/// exponential backoff.
pub struct ListenSession {
    pool: PgPool,
    channel: String,
    min_backoff: Duration,
    max_backoff: Duration,
}

impl ListenSession {
    pub fn new(pool: PgPool, channel: impl Into<String>) -> Self {
        Self {
            pool,
            channel: channel.into(),
            min_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(30),
        }
    }

    /// Runs until the task is dropped. Each connection loss backs off and
    /// reconnects, re-seeding on the new connection.
    pub async fn run(self, mut handler: impl NotifyHandler) {
        let mut backoff = self.min_backoff;
        loop {
            match self.session(&mut handler, &mut backoff).await {
                Ok(never) => match never {},
                Err(err) => tracing::warn!(channel = %self.channel, ?err, "listen session lost"),
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(self.max_backoff);
        }
    }

    /// One connection lifecycle: subscribe, seed, then drain until the
    /// connection drops. When the listener silently reconnects, `try_recv`
    /// yields `None` and the seed re-runs in place to recover any missed
    /// NOTIFY.
    async fn session(
        &self,
        handler: &mut impl NotifyHandler,
        backoff: &mut Duration,
    ) -> Result<Infallible> {
        let mut listener = PgListener::connect_with(&self.pool)
            .await
            .context("connect listener")?;
        listener
            .listen(&self.channel)
            .await
            .context("subscribe channel")?;
        handler.seed().await.context("seed")?;
        // The connection is healthy, so a later drop should retry promptly.
        *backoff = self.min_backoff;
        loop {
            match listener.try_recv().await.context("receive notification")? {
                Some(notification) => handler
                    .on_notify(notification.payload())
                    .await
                    .context("handle notification")?,
                None => handler.seed().await.context("reseed after reconnect")?,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{ListenSession, NotifyHandler},
        anyhow::Result,
        async_trait::async_trait,
        sqlx::PgPool,
        std::{
            sync::{Arc, Mutex},
            time::Duration,
        },
        tokio::sync::oneshot,
    };

    type Log = Arc<Mutex<Vec<String>>>;

    /// Records `seed` and `on_notify`. `seed` blocks on a gate so the test can
    /// fire a NOTIFY in the subscribe-to-seed window and prove it is delivered.
    struct Recorder {
        log: Log,
        started: Option<oneshot::Sender<()>>,
        gate: Option<oneshot::Receiver<()>>,
    }

    #[async_trait]
    impl NotifyHandler for Recorder {
        async fn seed(&mut self) -> Result<()> {
            self.log.lock().unwrap().push("seed".to_owned());
            if let Some(started) = self.started.take() {
                let _ = started.send(());
            }
            if let Some(gate) = self.gate.take() {
                let _ = gate.await;
            }
            Ok(())
        }

        async fn on_notify(&mut self, payload: &str) -> Result<()> {
            self.log.lock().unwrap().push(format!("notify:{payload}"));
            Ok(())
        }
    }

    async fn wait_for(log: &Log, n: usize) {
        for _ in 0..200 {
            if log.lock().unwrap().len() >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("timed out, log = {:?}", log.lock().unwrap());
    }

    #[tokio::test]
    #[ignore = "needs a local postgres, run manually, kept out of CI"]
    async fn delivers_notify_fired_during_seed() {
        const CHANNEL: &str = "autopilot_svm_listen_test";
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let log: Log = Arc::default();
        let (started_tx, started_rx) = oneshot::channel();
        let (gate_tx, gate_rx) = oneshot::channel();

        let session = ListenSession::new(pool.clone(), CHANNEL);
        let task = tokio::spawn(session.run(Recorder {
            log: log.clone(),
            started: Some(started_tx),
            gate: Some(gate_rx),
        }));

        // LISTEN is active once seed starts: fire the gap NOTIFY now.
        started_rx.await.unwrap();
        sqlx::query(r#"SELECT pg_notify($1, $2)"#)
            .bind(CHANNEL)
            .bind("A")
            .execute(&pool)
            .await
            .unwrap();

        // release seed, then fire a post-seed NOTIFY.
        gate_tx.send(()).unwrap();
        sqlx::query(r#"SELECT pg_notify($1, $2)"#)
            .bind(CHANNEL)
            .bind("B")
            .execute(&pool)
            .await
            .unwrap();

        wait_for(&log, 3).await;
        task.abort();
        assert_eq!(*log.lock().unwrap(), ["seed", "notify:A", "notify:B"]);
    }
}
