use super::Postgres;
use anyhow::Result;
use chrono::{DateTime, Utc};
use database::auction::AuctionId;
use futures::{StreamExt, TryStreamExt};
use model::{auction::Auction, order::Order};
use shared::db_order_conversions::full_order_into_model_order;

pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub latest_settlement_block: u64,
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
