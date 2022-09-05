use anyhow::Result;
use model::auction::{Auction, AuctionWithId};

impl super::Postgres {
    pub async fn most_recent_auction(&self) -> Result<Option<AuctionWithId>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_most_recent_auction"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let (id, json) = match database::auction::load_most_recent(&mut ex).await? {
            Some(inner) => inner,
            None => return Ok(None),
        };
        let auction: Auction = serde_json::from_value(json)?;
        let auction = AuctionWithId { id, auction };
        Ok(Some(auction))
    }
}
