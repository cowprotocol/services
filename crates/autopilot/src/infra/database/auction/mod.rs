use {
    self::postgres::dto,
    super::Database,
    crate::domain::{self},
};

pub mod postgres;

impl Database {
    /// Saves the given auction to the database and returns the ID of the
    /// saved auction.
    ///
    /// There is only one auction in the database at any given time.
    pub async fn set(&self, auction: domain::Auction) -> Result<domain::AuctionId, Error> {
        let auction = dto::from_domain(auction);
        self.db.save(auction).await.map_err(Error::DbError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to write data to database")]
    DbError(#[from] anyhow::Error),
}
