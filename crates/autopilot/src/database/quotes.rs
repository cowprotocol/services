use {
    super::Postgres,
    anyhow::{Context, Result},
    shared::maintenance::Maintaining,
    sqlx::types::chrono::{DateTime, Utc},
};

impl Postgres {
    pub async fn remove_expired_quotes(&self, max_expiry: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["remove_expired_quotes"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::quotes::remove_expired_quotes(&mut ex, max_expiry).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Maintaining for Postgres {
    async fn run_maintenance(&self) -> Result<()> {
        self.remove_expired_quotes(Utc::now())
            .await
            .context("fee measurement maintenance error")
    }

    fn name(&self) -> &str {
        "Postgres"
    }
}
