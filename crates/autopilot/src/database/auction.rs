use {
    super::Postgres,
    anyhow::{Context, Result},
    database::{auction::AuctionId, quotes::QuoteKind},
    futures::{StreamExt, TryStreamExt},
    model::{auction::Auction, order::Order},
};

pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub latest_settlement_block: u64,
}
use {
    chrono::{DateTime, Utc},
    model::quote::QuoteId,
    shared::{
        db_order_conversions::full_order_into_model_order,
        event_storing_helpers::{create_db_search_parameters, create_quote_row},
        order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
    },
};

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

impl Postgres {
    pub async fn solvable_orders(
        &self,
        min_valid_to: u32,
        min_surplus_fee_timestamp: DateTime<Utc>,
    ) -> Result<SolvableOrders> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["solvable_orders"])
            .start_timer();

        let mut ex = self.0.begin().await?;
        let orders = database::orders::solvable_orders(
            &mut ex,
            min_valid_to as i64,
            min_surplus_fee_timestamp,
        )
        .map(|result| match result {
            Ok(order) => full_order_into_model_order(order),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await?;
        let latest_settlement_block =
            database::orders::latest_settlement_block(&mut ex).await? as u64;
        Ok(SolvableOrders {
            orders,
            latest_settlement_block,
        })
    }

    pub async fn replace_current_auction(&self, auction: &Auction) -> Result<AuctionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_auction"])
            .start_timer();

        let data = serde_json::to_value(auction)?;
        let mut ex = self.0.begin().await?;
        database::auction::delete_all_auctions(&mut ex).await?;
        let id = database::auction::save(&mut ex, &data).await?;
        ex.commit().await?;
        Ok(id)
    }
}
