use {
    super::Postgres,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
    shared::{
        event_storing_helpers::create_db_search_parameters,
        order_quoting::{QuoteCompetition, QuoteData, QuoteSearchParameters, QuoteStoring},
        quote_storage::save_quote_competition,
    },
};

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteCompetition) -> Result<QuoteId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

        let mut tx = self.pool.begin().await?;
        let id = save_quote_competition(&mut tx, data, &self.domain_separator).await?;
        tx.commit().await?;
        Ok(id)
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let quote = database::quotes::get(&mut ex, id).await?;
        quote.map(TryFrom::try_from).transpose()
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

        let mut ex = self.pool.acquire().await?;
        let params = create_db_search_parameters(params, expiration);
        let quote = database::quotes::find(&mut ex, &params)
            .await
            .context("failed finding quote by parameters")?;
        quote
            .map(|quote| Ok((quote.id, quote.try_into()?)))
            .transpose()
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
