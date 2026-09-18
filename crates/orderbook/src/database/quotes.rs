use {
    super::Postgres,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
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

    async fn get_next_auction_id(&self) -> Result<i64> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_next_auction_id"])
            .start_timer();
        let mut ex = self.pool.acquire().await?;
        database::auction::get_next_auction_id(&mut ex)
            .await
            .context("failed to fetch next auction_id")
    }
}
