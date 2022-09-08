use super::Postgres;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use database::{
    byte_array::ByteArray,
    quotes::{Quote as QuoteRow, QuoteSearchParameters as DbQuoteSearchParameters},
};
use model::quote::QuoteId;
use number_conversions::u256_to_big_decimal;
use shared::{
    db_order_conversions::order_kind_into,
    order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
};

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteData) -> Result<Option<QuoteId>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let row = QuoteRow {
            id: Default::default(),
            sell_token: ByteArray(data.sell_token.0),
            buy_token: ByteArray(data.buy_token.0),
            sell_amount: u256_to_big_decimal(&data.quoted_sell_amount),
            buy_amount: u256_to_big_decimal(&data.quoted_buy_amount),
            gas_amount: data.fee_parameters.gas_amount,
            gas_price: data.fee_parameters.gas_price,
            sell_token_price: data.fee_parameters.sell_token_price,
            order_kind: order_kind_into(data.kind),
            expiration_timestamp: data.expiration,
            quote_kind: data.quote_kind,
        };
        let id = database::quotes::save(&mut ex, &row).await?;
        Ok(Some(id))
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
        let params = DbQuoteSearchParameters {
            sell_token: ByteArray(params.sell_token.0),
            buy_token: ByteArray(params.buy_token.0),
            sell_amount_0: u256_to_big_decimal(&params.sell_amount),
            sell_amount_1: u256_to_big_decimal(&(params.sell_amount + params.fee_amount)),
            buy_amount: u256_to_big_decimal(&params.buy_amount),
            kind: order_kind_into(params.kind),
            expiration,
            quote_kind: params.quote_kind,
        };
        let quote = database::quotes::find(&mut ex, &params)
            .await
            .context("failed finding quote by parameters")?;
        quote
            .map(|quote| Ok((quote.id, quote.try_into()?)))
            .transpose()
    }
}
