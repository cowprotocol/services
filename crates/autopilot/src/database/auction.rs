use {
    super::Postgres,
    crate::{boundary, domain, infra::persistence::dto},
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    futures::{StreamExt, TryStreamExt},
    model::{order::Order, quote::QuoteId},
    num::ToPrimitive,
    shared::{
        db_order_conversions::full_order_into_model_order,
        event_storing_helpers::{
            create_db_search_parameters,
            create_quote_interactions_insert_data,
            create_quote_row,
        },
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
    sqlx::Acquire,
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
}
