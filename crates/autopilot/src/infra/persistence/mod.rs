use {
    crate::{boundary, database::Postgres, domain},
    chrono::Utc,
    std::{collections::HashMap, sync::Arc},
    tokio::time::Instant,
    tracing::Instrument,
};

pub mod cli;
pub mod dto;

pub struct Persistence {
    s3: Option<s3::Uploader>,
    postgres: Arc<Postgres>,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>, postgres: Arc<Postgres>) -> Self {
        Self {
            s3: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
            postgres,
        }
    }

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

    /// Saves the competition data to the DB
    pub async fn save_competition(&self, competition: &boundary::Competition) -> Result<(), Error> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(Error::DbError)
    }

    /// Inserts the given events with the current timestamp into the DB.
    /// If this function encounters an error it will only be printed. More
    /// elaborate error handling is not necessary because this is just
    /// debugging information.
    pub fn store_order_events(&self, events: Vec<(domain::OrderUid, boundary::OrderEventLabel)>) {
        let db = self.postgres.clone();
        tokio::spawn(
            async move {
                let start = Instant::now();
                match boundary::store_order_events(&db, &events, Utc::now()).await {
                    Ok(_) => {
                        tracing::debug!(elapsed=?start.elapsed(), events_count=events.len(), "stored order events");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to insert order events");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }

    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &domain::OrderUid>,
    ) -> Result<HashMap<domain::OrderUid, domain::Quote>, Error> {
        let mut quotes = HashMap::new();
        for (id, quote) in self.postgres.read_quotes(orders).await? {
            quotes.insert(id, dto::quote::into_domain(quote)?);
        }
        Ok(quotes)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read data from database")]
    DbError(#[from] anyhow::Error),
    #[error(transparent)]
    Conversion(#[from] dto::quote::AmountOverflow),
}
