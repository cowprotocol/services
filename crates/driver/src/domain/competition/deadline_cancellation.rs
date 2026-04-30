use {
    crate::domain::time::Remaining,
    chrono::{DateTime, Utc},
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
};

/// Cancels a token when the configured deadline is reached, if dropped before
/// the deadline is reached, will not trigger token cancellation.
#[derive(Debug)]
pub struct DeadlineCancellation {
    // The cancellation token
    token: CancellationToken,
    // Task that cancels the cancellation token
    guard: JoinHandle<()>,
}

impl DeadlineCancellation {
    /// Spawns a task that cancels the token at `deadline`.
    pub fn new(deadline: DateTime<Utc>) -> Self {
        let token = CancellationToken::new();
        let guard = match deadline.remaining() {
            Ok(remaining) => {
                let cancel = token.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(remaining).await;
                    cancel.cancel();
                })
            }
            Err(_) => {
                token.cancel();
                tokio::spawn(async {}) // no-op
            }
        };
        Self { token, guard }
    }

    /// Returns the cancellation token.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }
}

impl Drop for DeadlineCancellation {
    fn drop(&mut self) {
        self.guard.abort();
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::domain::competition::deadline_cancellation::DeadlineCancellation,
        chrono::Utc,
        std::time::Duration,
    };

    #[tokio::test]
    async fn cancels_after_deadline() {
        let deadline = Utc::now() + chrono::Duration::seconds(1);
        let cancellation = DeadlineCancellation::new(deadline);

        tokio::time::timeout(Duration::from_secs(2), cancellation.token().cancelled())
            .await
            .expect("deadline cancellation");
    }

    #[tokio::test]
    async fn not_cancelled_before_deadline() {
        let deadline = Utc::now() + chrono::Duration::seconds(60);
        let cancellation = DeadlineCancellation::new(deadline);

        assert!(!cancellation.token().is_cancelled());
    }

    #[tokio::test]
    async fn cancels_immediately_when_expired() {
        let deadline = Utc::now() - chrono::Duration::milliseconds(1);
        let cancellation = DeadlineCancellation::new(deadline);

        assert!(cancellation.token().is_cancelled());
    }

    #[tokio::test]
    async fn drop_aborts_timer_without_cancelling_token() {
        let deadline = Utc::now() + chrono::Duration::seconds(1);
        let cancellation = DeadlineCancellation::new(deadline);
        let token = cancellation.token();
        drop(cancellation);
        tokio::time::sleep(Duration::from_secs(2)).await;

        assert!(!token.is_cancelled());
    }
}
