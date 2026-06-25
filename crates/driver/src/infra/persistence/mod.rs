use {
    crate::{
        domain::competition::auction::Id,
        infra::{config::file, solver::Config},
    },
    bytes::Bytes,
    std::sync::Arc,
    tokio::sync::oneshot,
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

    /// Whether auction archival to S3 is configured. When it isn't, the solver
    /// request body never needs to be fully materialized and can be streamed.
    pub fn archives_enabled(&self) -> bool {
        self.s3.is_some()
    }

    /// Archives the auction body to S3 (fire and forget). The body is
    /// gzip-compressed while it is streamed to the solver, so the full
    /// uncompressed JSON is never held in memory; the compressed bytes arrive
    /// through `compressed` once serialization finishes. An error on the
    /// receiver means the request was aborted, so there is nothing to archive.
    pub fn archive_auction_gzipped(&self, auction_id: Id, compressed: oneshot::Receiver<Bytes>) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        tokio::spawn(
            async move {
                let Ok(compressed) = compressed.await else {
                    return;
                };
                match uploader
                    .upload_gzipped(auction_id.to_string(), compressed)
                    .await
                {
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
