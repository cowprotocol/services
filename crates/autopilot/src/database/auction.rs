use {
    super::Postgres,
    crate::{boundary, domain, infra::persistence::dto},
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    database::orders::FullOrder,
    ethcontract::jsonrpc::futures_util::stream::BoxStream,
    futures::{StreamExt, TryStreamExt},
    model::{order::Order, quote::QuoteId},
    shared::{
        db_order_conversions::full_order_into_model_order,
        event_storing_helpers::{create_db_search_parameters, create_quote_row},
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
        let row = create_quote_row(data);
        let id = database::quotes::save(&mut ex, &row).await?;
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
}

impl Postgres {
    pub async fn solvable_orders(&self, min_valid_to: u32) -> Result<boundary::SolvableOrders> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["solvable_orders"])
            .start_timer();

        self.fetch_orders_data(|ex| database::orders::solvable_orders(ex, min_valid_to as i64))
            .await
    }

    pub async fn orders_after(
        &self,
        after_timestamp: DateTime<Utc>,
        min_valid_to: u32,
    ) -> Result<boundary::SolvableOrders> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["orders_after"])
            .start_timer();

        self.fetch_orders_data(|ex| {
            database::orders::full_orders_after(ex, after_timestamp, min_valid_to as i64)
        })
        .await
    }

    pub async fn replace_current_auction(&self, auction: &dto::Auction) -> Result<dto::AuctionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_current_auction"])
            .start_timer();

        let data = serde_json::to_value(auction)?;
        let mut ex = self.pool.begin().await?;
        database::auction::delete_all_auctions(&mut ex).await?;
        let id = database::auction::save(&mut ex, &data).await?;
        ex.commit().await?;
        Ok(id)
    }

    async fn fetch_orders_data<F>(&self, orders_fn: F) -> Result<boundary::SolvableOrders>
    where
        F: FnOnce(&mut PgConnection) -> BoxStream<'_, std::result::Result<FullOrder, sqlx::Error>>,
    {
        let mut ex = self.pool.begin().await?;
        // Set the transaction isolation level to REPEATABLE READ
        // so the both SELECT queries below are executed in the same database snapshot
        // taken at the moment before the first query is executed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(ex.deref_mut())
            .await?;

        let orders: HashMap<domain::OrderUid, Order> = orders_fn(&mut ex)
            .map(|result| match result {
                Ok(order) => full_order_into_model_order(order)
                    .map(|order| (domain::OrderUid(order.metadata.uid.0), order)),
                Err(err) => Err(anyhow::Error::from(err)),
            })
            .try_collect()
            .await?;
        let latest_settlement_block =
            database::orders::latest_settlement_block(&mut ex).await? as u64;
        let quotes = self.read_quotes(orders.keys()).await?;
        Ok(boundary::SolvableOrders {
            orders,
            quotes,
            latest_settlement_block,
        })
    }
}
