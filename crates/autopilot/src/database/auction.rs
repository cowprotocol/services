use {
    super::Postgres,
    crate::{boundary, domain, infra::persistence::dto},
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    futures::{StreamExt, TryStreamExt},
    model::{order::Order, quote::QuoteId},
    shared::{
        db_order_conversions::full_order_into_model_order,
        event_storing_helpers::{create_db_search_parameters, create_quote_row},
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
    std::ops::DerefMut,
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

        let mut ex = self.pool.begin().await?;
        // Set the transaction isolation level to REPEATABLE READ
        // so the both SELECT queries below are executed in the same database snapshot
        // taken at the moment before the first query is executed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(ex.deref_mut())
            .await?;
        let orders: Vec<Order> = database::orders::solvable_orders(&mut ex, min_valid_to as i64)
            .map(|result| match result {
                Ok(order) => full_order_into_model_order(order),
                Err(err) => Err(anyhow::Error::from(err)),
            })
            .try_collect()
            .await?;
        let latest_settlement_block =
            database::orders::latest_settlement_block(&mut ex).await? as u64;
        let quotes = self
            .read_quotes(
                orders
                    .iter()
                    .map(|order| domain::OrderUid(order.metadata.uid.0))
                    .collect::<Vec<_>>()
                    .iter(),
            )
            .await?;
        Ok(boundary::SolvableOrders {
            orders,
            quotes,
            latest_settlement_block,
        })
    }

    pub async fn replace_current_auction(&self, auction: &dto::Auction) -> Result<dto::AuctionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_current_auction"])
            .start_timer();

        let data = serde_json::to_value(auction)?;
        let mut ex = self.pool.begin().await?;
        database::auction::delete_all_auctions(&mut ex).await?;
        let mut id = database::auction::save(&mut ex, &data).await?;
        if id < i64::MAX / 2 {
            // update the auction id in the `auctions` table
            database::auction::update_current_auction_id(&mut ex, i64::MAX / 2).await?;
            // return the updated value
            id = i64::MAX / 2;
        }
        ex.commit().await?;
        Ok(id)
    }
}
