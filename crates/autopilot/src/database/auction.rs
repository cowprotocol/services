use {
    super::Postgres,
    crate::{boundary, domain, infra::persistence::dto},
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    futures::{StreamExt, TryStreamExt},
    model::{interaction::InteractionData, order::Order, quote::QuoteId},
    num::ToPrimitive,
    number::conversions::big_decimal_to_u256,
    primitive_types::H160,
    shared::{
        db_order_conversions::full_order_into_model_order,
        event_storing_helpers::{
            create_db_search_parameters,
            create_quote_interactions_insert_data,
            create_quote_row,
        },
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
    sqlx::PgConnection,
    std::{collections::HashMap, ops::DerefMut},
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
        let id = database::quotes::save(&mut ex, &row).await?;
        if !data.interactions.is_empty() {
            let interactions = create_quote_interactions_insert_data(id, &data)?;
            database::quotes::insert_quote_interactions(&mut ex, &interactions).await?;
        }
        Ok(id)
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let quote = database::quotes::get(&mut ex, id).await?;
        let quote_interactions = Self::get_quote_interactions(&mut ex, id).await?;
        Ok(quote
            .map(QuoteData::try_from)
            .transpose()?
            .map(|mut quote_data| {
                quote_data.interactions = quote_interactions;
                quote_data
            }))
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
        if let Some(quote) = quote {
            let quote_id = quote.id;
            let quote_interactions = Self::get_quote_interactions(&mut ex, quote_id).await?;
            let mut quote_data = QuoteData::try_from(quote)?;
            quote_data.interactions = quote_interactions;
            Ok(Some((quote_id, quote_data)))
        } else {
            Ok(None)
        }
    }
}

impl Postgres {
    pub async fn all_solvable_orders(&self, min_valid_to: u32) -> Result<boundary::SolvableOrders> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["solvable_orders"])
            .start_timer();

        let start = chrono::offset::Utc::now();
        let mut ex = self.pool.begin().await?;
        // Set the transaction isolation level to REPEATABLE READ
        // so the both SELECT queries below are executed in the same database snapshot
        // taken at the moment before the first query is executed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(ex.deref_mut())
            .await?;
        let orders: HashMap<domain::OrderUid, Order> =
            database::orders::solvable_orders(&mut ex, i64::from(min_valid_to))
                .map(|result| match result {
                    Ok(order) => full_order_into_model_order(order)
                        .map(|order| (domain::OrderUid(order.metadata.uid.0), order)),
                    Err(err) => Err(anyhow::Error::from(err)),
                })
                .try_collect()
                .await?;
        let latest_settlement_block = database::orders::latest_settlement_block(&mut ex)
            .await?
            .to_u64()
            .context("latest_settlement_block is not u64")?;
        let quotes = self.read_quotes(orders.keys()).await?;
        Ok(boundary::SolvableOrders {
            orders,
            quotes,
            latest_settlement_block,
            fetched_from_db: start,
        })
    }

    pub async fn replace_current_auction(
        &self,
        auction: &dto::RawAuctionData,
    ) -> Result<dto::AuctionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_current_auction"])
            .start_timer();

        let data = serde_json::to_value(auction)?;
        let mut ex = self.pool.acquire().await?;
        let id = database::auction::replace_auction(&mut ex, &data).await?;
        Ok(id)
    }

    async fn get_quote_interactions(
        ex: &mut PgConnection,
        quote_id: QuoteId,
    ) -> Result<Vec<InteractionData>> {
        database::quotes::get_quote_interactions(ex, quote_id)
            .await?
            .iter()
            .map(|data| {
                Ok(InteractionData {
                    target: H160(data.target.0),
                    value: big_decimal_to_u256(&data.value)
                        .context("quote interaction value is not a valid U256")?,
                    call_data: data.call_data.clone(),
                })
            })
            .collect::<Result<Vec<InteractionData>>>()
    }
}
