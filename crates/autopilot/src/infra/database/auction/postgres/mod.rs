use {crate::database::Postgres, anyhow::Result};

pub mod dto;

impl Postgres {
    /// Saves the given auction to the database and returns the ID of the
    /// auction.
    pub async fn save(&self, auction: dto::Auction) -> Result<dto::AuctionId> {
        let data = serde_json::to_value(auction)?;
        let mut ex = self.pool.begin().await?;
        database::auction::delete_all_auctions(&mut ex).await?;
        let id = database::auction::save(&mut ex, &data).await?;
        ex.commit().await?;
        Ok(id)
    }
}
