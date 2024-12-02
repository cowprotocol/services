use {
    super::Postgres,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
    shared::{
        event_storing_helpers::{
            create_db_search_parameters,
            create_quote_interactions_insert_data,
            create_quote_row,
        },
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
    sqlx::Acquire,
};

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteData) -> Result<QuoteId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let row = create_quote_row(&data);

        let mut transaction = ex.begin().await?;
        let id = database::quotes::save(&mut transaction, &row).await?;
        if !data.interactions.is_empty() {
            let interactions = create_quote_interactions_insert_data(id, &data)?;
            database::quotes::insert_quote_interactions(&mut transaction, &interactions).await?;
        }
        transaction.commit().await.context("commit")?;
        Ok(id)
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;

        let query_result = database::quotes::get_quote_with_interactions(&mut ex, id).await?;

        Ok(query_result.map(QuoteData::try_from).transpose()?)
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

        let query_result = database::quotes::find_quote_with_interactions(&mut ex, &params)
            .await
            .context("failed finding quote by parameters")?;

        query_result
            .map(|query_result| Ok((query_result.id, query_result.try_into()?)))
            .transpose()
    }
}
