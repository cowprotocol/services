use {database::auction::AuctionId, model::auction::Auction, tracing::Instrument};
pub mod s3;

pub struct Persistence {
    inner: Option<s3::Uploader>,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>) -> Self {
        Self {
            inner: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
        }
    }

    /// Persists the given auction in a background task.
    pub fn store_auction(&self, id: AuctionId, instance: &Auction) {
        if let Some(uploader) = self.inner.clone() {
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
