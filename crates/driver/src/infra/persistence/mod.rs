use {
    crate::{
        domain::competition::auction::Id,
        infra::{config::file, solver::Config},
    },
    std::sync::Arc,
    tracing::Instrument,
};

#[derive(Clone, Debug, Default)]
pub struct S3 {
    /// Name of the AWS S3 bucket in which the auctions will be stored
    pub bucket: String,

    /// Prepended to the auction id to form the final instance filename on AWS
    /// S3 bucket. Something like "staging/mainnet/"
    pub prefix: String,
}

impl From<file::S3> for S3 {
    fn from(value: file::S3) -> Self {
        Self {
            bucket: value.bucket,
            prefix: value.prefix,
        }
    }
}

impl From<S3> for s3::Config {
    fn from(value: S3) -> Self {
        Self {
            bucket: value.bucket,
            filename_prefix: value.prefix,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Persistence {
    s3: Option<Arc<s3::Uploader>>,
}

impl Persistence {
    pub async fn build(config: &Config) -> Self {
        if let Some(s3) = &config.s3 {
            Self {
                s3: Some(Arc::new(s3::Uploader::new(s3.clone().into()).await)),
            }
        } else {
            Self { s3: None }
        }
    }

    /// Saves the given auction with liquidity with fire and forget mentality
    /// (non-blocking operation)
    pub fn archive_auction(&self, auction_id: Id, body: &str) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        let body = body.to_string();
        tokio::spawn(
            async move {
                match uploader.upload(auction_id.to_string(), body).await {
                    Ok(key) => {
                        tracing::debug!(?key, "uploaded auction with liquidity to s3");
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
