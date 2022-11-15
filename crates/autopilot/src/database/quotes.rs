use super::Postgres;
use anyhow::{Context, Result};
use database::quotes::QuoteKind;
use model::quote::QuoteId;
use shared::{
    event_storing_helpers::{create_db_search_parameters, create_quote_row},
    maintenance::Maintaining,
    order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
};
use sqlx::types::chrono::{DateTime, Utc};

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
}

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteData) -> Result<QuoteId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let row = create_quote_row(data);
        let id = database::quotes::save(&mut ex, &row).await?;
        Ok(id)
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let quote = database::quotes::get(&mut ex, id).await?;
        quote.map(TryFrom::try_from).transpose()
    }

    async fn find(
        &self,
        params: QuoteSearchParameters,
        expiration: DateTime<Utc>,
        quote_kind: QuoteKind,
    ) -> Result<Option<(QuoteId, QuoteData)>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["find_quote"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let params = create_db_search_parameters(params, expiration, quote_kind);
        let quote = database::quotes::find(&mut ex, &params)
            .await
            .context("failed finding quote by parameters")?;
        quote
            .map(|quote| Ok((quote.id, quote.try_into()?)))
            .transpose()
    }
}
