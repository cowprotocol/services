use {
    crate::{domain::competition::auction::Id, infra::solver::Config},
    serde::Deserialize,
    std::sync::Arc,
    tracing::Instrument,
};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct S3 {
    /// The s3_instance_upload_* arguments configure how auction instances
    /// should be uploaded to AWS S3.
    /// They must either all be set or all not set.
    pub bucket: String,

    /// Prepended to the auction id to form the final instance filename on S3.
    /// Something like "staging/mainnet/"
    pub prefix: String,
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
    pub fn archive_auction(&self, auction_id: Option<Id>, body: &str) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        let Some(id) = auction_id else {
            return;
        };
        let body = body.to_string();
        tokio::spawn(
            async move {
                match uploader.upload(id.to_string(), body).await {
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
