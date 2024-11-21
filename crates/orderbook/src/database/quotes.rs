use {
    super::Postgres,
    anyhow::Result,
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
    shared::{
        database_access::orders::*,
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
};

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteData) -> Result<QuoteId> {
        quote_save(&data, &super::Metrics::get().database_queries, &self.pool).await
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        quote_get(id, &super::Metrics::get().database_queries, &self.pool).await
    }

    async fn find(
        &self,
        params: QuoteSearchParameters,
        expiration: DateTime<Utc>,
    ) -> Result<Option<(QuoteId, QuoteData)>> {
        quote_find(
            &params,
            &expiration,
            &super::Metrics::get().database_queries,
            &self.pool,
        )
        .await
    }
}
