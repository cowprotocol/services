use {
    super::Postgres,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    futures::future::{BoxFuture, FutureExt},
    model::quote::QuoteId,
    price_estimation::QuoteIdGenerating,
    shared::{
        order_quoting::{QuoteCompetition, QuoteData, QuoteSearchParameters, QuoteStoring},
        quote_storage::{find_quote, get_quote, save_quote},
    },
};

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteCompetition) -> Result<QuoteId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

        save_quote(&self.pool, data).await
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        get_quote(&self.pool, id).await
    }

    async fn find(
        &self,
        params: QuoteSearchParameters,
        expiration: DateTime<Utc>,
    ) -> Result<Option<(QuoteId, QuoteData)>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["find_quote"])
            .start_timer();

        find_quote(&self.pool, params, expiration).await
    }

    async fn next_quote_id(&self) -> Result<QuoteId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["next_quote_id"])
            .start_timer();
        let mut ex = self.pool.acquire().await?;
        database::quotes::next_id(&mut ex)
            .await
            .context("failed to allocate next quote id")
    }
}

impl QuoteIdGenerating for Postgres {
    fn generate(&self, n: usize) -> BoxFuture<'_, Result<Vec<QuoteId>>> {
        async move {
            let _timer = super::Metrics::get()
                .database_queries
                .with_label_values(&["next_quote_id_generator"])
                .start_timer();
            let mut ex = self.pool.acquire().await?;
            database::quotes::next_ids(&mut ex, n)
                .await
                .context("failed to allocate quote ids")
        }
        .boxed()
    }
}
