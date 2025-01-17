use {
    futures::FutureExt,
    moka::future::Cache,
    reqwest::StatusCode,
    shared::request_sharing::BoxRequestSharing,
    std::sync::Arc,
    thiserror::Error,
    url::Url,
};

#[derive(Clone)]
pub struct AppDataFetcher(Arc<Inner>);

struct Inner {
    client: reqwest::Client,
    base_url: Url,
    request_sharing: BoxRequestSharing<
        super::AppDataHash,
        Result<Option<app_data::ValidatedAppData>, FetchingError>,
    >,
    app_data_validator: app_data::Validator,
    cache: Cache<super::AppDataHash, Option<app_data::ValidatedAppData>>,
}

impl AppDataFetcher {
    pub fn new(orderbook_url: Url) -> Self {
        Self(Arc::new(Inner {
            client: reqwest::Client::new(),
            base_url: orderbook_url,
            request_sharing: BoxRequestSharing::labelled("app_data".to_string()),
            app_data_validator: app_data::Validator::new(usize::MAX),
            cache: Cache::new(2_000),
        }))
    }

    pub async fn fetch(
        &self,
        app_data: super::AppDataHash,
    ) -> Result<Option<app_data::ValidatedAppData>, FetchingError> {
        if let Some(app_data) = self.0.cache.get(&app_data).await {
            return Ok(app_data.clone());
        }

        let app_data_fut = move |app_data: &super::AppDataHash| {
            let app_data = *app_data;
            let self_ = self.clone();

            async move {
                let url = self_
                    .0
                    .base_url
                    .join(&format!("v1/app_data/{:?}", app_data))?;
                let response = self_.0.client.get(url).send().await?;

                let validated_app_data = match response.status() {
                    StatusCode::NOT_FOUND => None,
                    _ => {
                        let bytes = response.error_for_status()?.bytes().await?;
                        Some(self_.0.app_data_validator.validate(&bytes)?)
                    }
                };

                self_
                    .0
                    .cache
                    .insert(app_data, validated_app_data.clone())
                    .await;

                Ok(validated_app_data)
            }
            .boxed()
        };

        self.0
            .request_sharing
            .shared_or_else(app_data, app_data_fut)
            .await
    }
}

#[derive(Error, Debug)]
pub enum FetchingError {
    #[error("unable to send a request: {0}")]
    Http(String),
    #[error("received invalid app data: {0}")]
    InvalidAppData(#[from] anyhow::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for FetchingError {
    fn from(err: reqwest::Error) -> Self {
        FetchingError::Http(err.to_string())
    }
}

impl From<url::ParseError> for FetchingError {
    fn from(err: url::ParseError) -> Self {
        FetchingError::Http(err.to_string())
    }
}

impl Clone for FetchingError {
    fn clone(&self) -> Self {
        match self {
            Self::Http(message) => Self::Http(message.clone()),
            Self::InvalidAppData(err) => Self::InvalidAppData(shared::clone_anyhow_error(err)),
            Self::Internal(message) => Self::Internal(message.clone()),
        }
    }
}
