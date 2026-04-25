use {
    crate::domain::time::Remaining,
    chrono::{DateTime, Utc},
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
};

/// Struct used to cancel a token when the auction driver deadline is reached.
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
        let cancel = token.clone();

        let guard = tokio::spawn(async move {
            if let Ok(remaining) = deadline.remaining() {
                tokio::time::sleep(remaining).await;
            }

            cancel.cancel();
        });

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
        let deadline = Utc::now() + chrono::Duration::milliseconds(20);
        let cancellation = DeadlineCancellation::new(deadline);

        tokio::time::timeout(Duration::from_secs(1), cancellation.token().cancelled())
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

        tokio::time::timeout(Duration::from_secs(1), cancellation.token().cancelled())
            .await
            .expect("expired deadline should cancel");
    }
}
