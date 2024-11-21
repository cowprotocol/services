use {
    crate::{
        event_storing_helpers::{create_db_search_parameters, create_quote_row},
        order_quoting::{QuoteData, QuoteSearchParameters},
    },
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
    sqlx::PgPool,
};

pub async fn quote_save(
    data: &QuoteData,
    metrics: &prometheus::HistogramVec,
    pool: &PgPool,
) -> Result<QuoteId> {
    let _timer = metrics.with_label_values(&["save_quote"]).start_timer();

    let mut ex = pool.acquire().await?;
    let row = create_quote_row(data);
    let id = database::quotes::save(&mut ex, &row).await?;
    Ok(id)
}

pub async fn quote_get(
    id: QuoteId,
    metrics: &prometheus::HistogramVec,
    pool: &PgPool,
) -> Result<Option<QuoteData>> {
    let _timer = metrics.with_label_values(&["get_quote"]).start_timer();

    let mut ex = pool.acquire().await?;
    let quote = database::quotes::get(&mut ex, id).await?;
    quote.map(TryFrom::try_from).transpose()
}

pub async fn quote_find(
    params: &QuoteSearchParameters,
    expiration: &DateTime<Utc>,
    metrics: &prometheus::HistogramVec,
    pool: &PgPool,
) -> Result<Option<(QuoteId, QuoteData)>> {
    let _timer = metrics.with_label_values(&["find_quote"]).start_timer();

    let mut ex = pool.acquire().await?;
    let params = create_db_search_parameters(params, expiration);
    let quote = database::quotes::find(&mut ex, &params)
        .await
        .context("failed finding quote by parameters")?;
    quote
        .map(|quote| Ok((quote.id, quote.try_into()?)))
        .transpose()
}
