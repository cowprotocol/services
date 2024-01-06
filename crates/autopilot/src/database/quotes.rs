use {
    super::Postgres,
    crate::domain::OrderUid,
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    shared::maintenance::Maintaining,
    sqlx::types::chrono::{DateTime, Utc},
    std::collections::HashMap,
};

impl Postgres {
    pub async fn remove_expired_quotes(&self, max_expiry: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["remove_expired_quotes"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::quotes::remove_expired_quotes(&mut ex, max_expiry).await?;
        Ok(())
    }

    /// Get quotes for all orders in the auction.
    ///
    /// Doens't guarantee that all orders have quotes.
    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &OrderUid>,
    ) -> Result<HashMap<OrderUid, database::orders::Quote>> {
        let mut ex = self.pool.acquire().await?;
        let mut quotes = HashMap::new();
        for order in orders {
            let order_uid = ByteArray(order.0);
            if let Some(quote) = database::orders::read_quote(&mut ex, &order_uid).await? {
                quotes.insert(*order, quote);
            }
        }
        Ok(quotes)
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
