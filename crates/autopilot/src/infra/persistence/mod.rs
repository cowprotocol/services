use {
    crate::database::Postgres,
    database::auction::AuctionId,
    model::auction::Auction,
    std::sync::Arc,
    tracing::Instrument,
};

pub mod cli;
pub mod fee_policy;

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

    /// Persists the given auction in a background task.
    pub fn store_auction(&self, id: AuctionId, instance: &Auction) {
        if let Some(uploader) = self.s3.clone() {
            let instance = instance.clone();
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
