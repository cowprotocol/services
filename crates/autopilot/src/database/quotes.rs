use {
    super::Postgres,
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, orders::Quote},
    model::{auction::Auction, order::OrderUid},
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

    pub async fn read_quotes(&self, auction: &Auction) -> Result<HashMap<OrderUid, Quote>> {
        let mut quote_tasks = Vec::new();
        for order in &auction.orders {
            let mut ex = self.pool.acquire().await?;
            let order_uid = ByteArray(order.metadata.uid.0);
            let quote_task = tokio::spawn(async move {
                database::orders::read_quote(&mut ex, &order_uid)
                    .await
                    .ok()?
            });
            quote_tasks.push((order.metadata.uid, quote_task));
        }
        let mut quotes_map = HashMap::new();
        for (order_uid, quote_task) in quote_tasks {
            let quote = quote_task
                .await?
                .ok_or(anyhow::anyhow!("failed to parse quote"))?;
            quotes_map.insert(order_uid, quote);
        }

        Ok(quotes_map)
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
