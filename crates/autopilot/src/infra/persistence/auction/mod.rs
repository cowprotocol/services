use {crate::domain, tracing::Instrument};

pub mod dto;

impl super::Persistence {
    /// Saves the given auction to storage for debugging purposes.
    ///
    /// There is no intention to retrieve this data programmatically.
    pub fn archive_auction(&self, id: domain::AuctionId, instance: &domain::Auction) {
        if let Some(uploader) = self.s3.clone() {
            let instance = dto::from_domain(instance.clone());
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

    /// There is always only one `current` auction.
    ///
    /// This method replaces the current auction with the given one.
    pub async fn replace_current_auction(
        &self,
        auction: domain::Auction,
    ) -> Result<domain::AuctionId, Error> {
        self.postgres
            .replace_current_auction(&dto::from_domain(auction))
            .await
            .map_err(Error::DbError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to write data to database")]
    DbError(#[from] anyhow::Error),
}
