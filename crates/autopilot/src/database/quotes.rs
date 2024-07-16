use {
    super::Postgres,
    crate::{
        domain::{self},
        infra::persistence::dto,
    },
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
    /// Doesn't guarantee that all orders have quotes.
    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &domain::OrderUid>,
    ) -> Result<HashMap<domain::OrderUid, domain::Quote>> {
        let mut ex = self.pool.acquire().await?;
        let order_uids: Vec<_> = orders.map(|uid| ByteArray(uid.0)).collect();
        let quotes: HashMap<_, _> = database::orders::read_quotes(&mut ex, &order_uids)
            .await?
            .into_iter()
            .filter_map(|quote| {
                let order_uid = domain::OrderUid(quote.order_uid.0);
                dto::quote::into_domain(quote)
                    .map_err(|err| {
                        tracing::warn!(?order_uid, ?err, "failed to convert quote from db")
                    })
                    .ok()
                    .map(|quote| (order_uid, quote))
            })
            .collect();

        // Log warnings for missing quotes
        for order_uid in order_uids
            .iter()
            .filter(|uid| !quotes.contains_key(&domain::OrderUid(uid.0)))
        {
            tracing::warn!(?order_uid, "quote not found for order");
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
