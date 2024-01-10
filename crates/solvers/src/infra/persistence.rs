use {crate::domain, tracing::Instrument};

pub struct Persistence {
    boundary_auction_upload: s3::Uploader,
}

impl Persistence {
    pub fn new(boundary_auction_upload: s3::Uploader) -> Self {
        Self {
            boundary_auction_upload,
        }
    }

    pub fn store_boundary(
        &self,
        id: domain::auction::Id,
        instance: &shared::http_solver::model::BatchAuctionModel,
    ) {
        let instance = instance.clone();
        let uploader = self.boundary_auction_upload.clone();
        tokio::spawn(
            async move {
                match uploader.upload(id.to_string(), &instance).await {
                    Ok(key) => {
                        tracing::info!(?key, "uploaded legacy auction to s3");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to upload legacy auction to s3");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}
