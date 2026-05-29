use {
    crate::dto,
    anyhow::{Context, Result},
};

impl super::Postgres {
    pub async fn most_recent_auction(&self) -> Result<Option<dto::AuctionWithId>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_most_recent_auction"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let (id, json) = match database::auction::load_most_recent(&mut ex).await? {
            Some(inner) => inner,
            None => return Ok(None),
        };
        let auction: dto::Auction = serde_json::from_value(json)?;
        let auction = dto::AuctionWithId { id, auction };
        Ok(Some(auction))
    }

    pub async fn last_used_auction_id(&self) -> Result<Option<i64>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["last_used_auction_id"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::auction::last_used_auction_id(&mut ex)
            .await
            .context("could not fetch last used auction_id")
    }
}
