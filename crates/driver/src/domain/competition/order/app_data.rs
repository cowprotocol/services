use {
    crate::util::Bytes,
    derive_more::From,
    futures::FutureExt,
    moka::future::Cache,
    reqwest::StatusCode,
    shared::request_sharing::BoxRequestSharing,
    std::sync::Arc,
    thiserror::Error,
    url::Url,
};

/// A struct for retrieving order's full app-data by its hash from a remote
/// service, with support for caching and deduplicating concurrent requests.
///
/// Ensures efficient access to application data by:
/// - Caching results to avoid redundant network requests.
/// - Sharing ongoing requests to prevent duplicate fetches for the same
///   `app_data`.
/// - Validating fetched app data.
///
/// LRU cache is used since only ~2% of app-data is unique across all orders
/// meaning that the cache hit rate is expected to be high, so there is no need
/// for TTL cache.
#[derive(Clone)]
pub struct AppDataRetriever(Arc<Inner>);

struct Inner {
    client: reqwest::Client,
    base_url: Url,
    request_sharing:
        BoxRequestSharing<AppDataHash, Result<Option<app_data::ValidatedAppData>, FetchingError>>,
    app_data_validator: app_data::Validator,
    cache: Cache<AppDataHash, Option<app_data::ValidatedAppData>>,
}

impl AppDataRetriever {
    pub fn new(orderbook_url: Url, cache_size: u64) -> Self {
        Self(Arc::new(Inner {
            client: reqwest::Client::new(),
            base_url: orderbook_url,
            request_sharing: BoxRequestSharing::labelled("app_data".to_string()),
            app_data_validator: app_data::Validator::new(usize::MAX),
            cache: Cache::new(cache_size),
        }))
    }

    /// Retrieves the full app-data for the given `app_data` hash, if exists.
    pub async fn get(
        &self,
        app_data: AppDataHash,
    ) -> Result<Option<app_data::ValidatedAppData>, FetchingError> {
        if let Some(app_data) = self.0.cache.get(&app_data).await {
            return Ok(app_data.clone());
        }

        let app_data_fut = move |app_data: &AppDataHash| {
            let app_data = *app_data;
            let self_ = self.clone();

            async move {
                let url = self_
                    .0
                    .base_url
                    .join(&format!("v1/app_data/{:?}", app_data.0))?;
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

/// The app data associated with an order.
#[derive(Debug, Clone, From)]
#[cfg_attr(test, derive(PartialEq))]
pub enum AppData {
    /// App data hash.
    Hash(AppDataHash),
    /// Validated full app data.
    Full(Box<::app_data::ValidatedAppData>),
}

impl Default for AppData {
    fn default() -> Self {
        Self::Hash(Default::default())
    }
}

impl AppData {
    pub fn hash(&self) -> AppDataHash {
        match self {
            Self::Hash(hash) => *hash,
            Self::Full(data) => AppDataHash(data.hash.0.into()),
        }
    }

    pub fn flashloan(&self) -> Option<app_data::Flashloan> {
        match self {
            Self::Hash(_) => None,
            Self::Full(data) => data.protocol.flashloan.clone(),
        }
    }
}

impl From<[u8; APP_DATA_LEN]> for AppData {
    fn from(app_data_hash: [u8; APP_DATA_LEN]) -> Self {
        Self::Hash(app_data_hash.into())
    }
}

impl From<::app_data::ValidatedAppData> for AppData {
    fn from(value: ::app_data::ValidatedAppData) -> Self {
        Self::Full(Box::new(value))
    }
}

/// The length of the app data hash in bytes.
pub const APP_DATA_LEN: usize = 32;

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub struct AppDataHash(pub Bytes<[u8; APP_DATA_LEN]>);

impl From<[u8; APP_DATA_LEN]> for AppDataHash {
    fn from(inner: [u8; APP_DATA_LEN]) -> Self {
        Self(inner.into())
    }
}

impl From<AppDataHash> for [u8; APP_DATA_LEN] {
    fn from(app_data: AppDataHash) -> Self {
        app_data.0.into()
    }
}

#[derive(Error, Debug)]
pub enum FetchingError {
    #[error("error while sending a request: {0}")]
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
        FetchingError::Internal(err.to_string())
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
