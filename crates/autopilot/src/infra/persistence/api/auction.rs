use {
    crate::{
        domain,
        infra::{self, persistence::dto},
    },
    tracing::Instrument,
};

impl infra::Persistence {
    /// There is always only one `current` auction.
    ///
    /// This method replaces the current auction with the given one.
    ///
    /// If the given auction is successfully saved, it is also archived.
    pub async fn replace_current_auction(
        &self,
        auction: domain::Auction,
    ) -> Result<domain::AuctionId, Error> {
        self.postgres
            .replace_current_auction(&dto::auction::from_domain(auction.clone()))
            .await
            .map(|auction_id| {
                self.archive_auction(auction_id, auction);
                auction_id
            })
            .map_err(Error::DbError)
    }

    /// Saves the given auction to storage for debugging purposes.
    ///
    /// There is no intention to retrieve this data programmatically.
    fn archive_auction(&self, id: domain::AuctionId, instance: domain::Auction) {
        if let Some(uploader) = self.s3.clone() {
            let instance = dto::auction::from_domain(instance);
            tokio::spawn(
                async move {
                    match uploader.upload(id.to_string(), &instance).await {
                        Ok(key) => {
                            tracing::info!(?key, "uploaded auction to s3");
                        }
                        Err(err) => {
                            tracing::warn!(?err, "failed to upload auction to s3");
                        }
                    }
                }
                .instrument(tracing::Span::current()),
            );
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to write data to database")]
    DbError(#[from] anyhow::Error),
}
