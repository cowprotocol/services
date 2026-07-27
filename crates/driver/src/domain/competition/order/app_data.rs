use {
    alloy::primitives::FixedBytes,
    anyhow::Context,
    app_data::AppDataDocument,
    derive_more::From,
    futures::TryStreamExt,
    moka::future::Cache,
    reqwest::StatusCode,
    serde_json::error::Category,
    std::{
        collections::HashMap,
        io::{BufReader, Read},
        sync::Arc,
        time::Duration,
    },
    thiserror::Error,
    tokio_util::io::{StreamReader, SyncIoBridge},
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
    cache: Cache<AppDataHash, Option<Arc<app_data::ValidatedAppData>>>,
}

impl AppDataRetriever {
    pub fn new(orderbook_url: Url, cache_size: u64) -> Self {
        Self(Arc::new(Inner {
            client: reqwest::Client::builder()
                .tcp_keepalive(Duration::from_secs(60))
                .build()
                .expect("reqwest client built correctly"),
            base_url: orderbook_url,
            cache: Cache::new(cache_size),
        }))
    }

    /// Returns all values that are currently cached.
    pub fn get_cached(&self) -> HashMap<Arc<AppDataHash>, Arc<app_data::ValidatedAppData>> {
        self.0
            .cache
            .iter()
            .flat_map(|(key, value)| Some((key, value?)))
            .collect()
    }

    /// Parses and validates an app data document read from `reader`. Blocking
    /// on purpose: it's meant to run on the blocking pool since both the
    /// reads and the (CPU bound) parsing would otherwise stall the executor.
    fn load_and_parse_app_data(
        hash: AppDataHash,
        reader: impl Read,
    ) -> Result<Option<Arc<app_data::ValidatedAppData>>, FetchingError> {
        let document: AppDataDocument =
            serde_json::from_reader(reader).map_err(|err| match err.classify() {
                // the body could not be read completely
                Category::Io => FetchingError::Http(err.to_string()),
                _ => anyhow::Error::new(err)
                    .context("invalid app data document")
                    .into(),
            })?;

        if document.full_app_data == app_data::EMPTY {
            return Ok(None);
        }

        Ok(Some(Arc::new(app_data::ValidatedAppData {
            hash: app_data::AppDataHash(hash.0.0),
            protocol: app_data::parse(document.full_app_data.as_bytes())
                .context("invalid app data json")?,
            document: document.full_app_data,
        })))
    }

    /// Retrieves the full app-data for the given `app_data` hash, if it exists.
    /// HTTP requests needed to fetch the data are spawned in background tasks
    /// such that they eventually populate the cache even in case the caller
    /// stops awaiting the returned future.
    pub async fn get_cached_or_fetch(
        &self,
        app_data: &AppDataHash,
    ) -> Result<Option<Arc<app_data::ValidatedAppData>>, FetchingError> {
        if let Some(app_data) = self.0.cache.get(app_data).await {
            return Ok(app_data.clone());
        }

        let inner = self.0.clone();
        let app_data = *app_data;

        let fut = async move {
            let url = inner
                .base_url
                .join(&format!("api/v1/app_data/{:?}", app_data.0))?;
            let response = inner.client.get(url).send().await?;

            let validated_app_data = match response.status() {
                StatusCode::NOT_FOUND => None,
                _ => {
                    // Bridge the async body stream into a blocking reader so the document
                    // gets parsed incrementally on the blocking pool instead of buffering
                    // the entire response in memory first.
                    let stream = response.bytes_stream().map_err(std::io::Error::other);
                    let reader = SyncIoBridge::new(StreamReader::new(Box::pin(stream)));
                    tokio::task::spawn_blocking(move || {
                        Self::load_and_parse_app_data(app_data, BufReader::new(reader))
                    })
                    .await??
                }
            };

            inner
                .cache
                .insert(app_data, validated_app_data.clone())
                .await;

            Ok(validated_app_data)
        };

        tokio::task::spawn(fut).await?
    }
}

/// The app data associated with an order.
#[derive(Debug, Clone, From)]
#[cfg_attr(test, derive(PartialEq))]
pub enum AppData {
    /// App data hash.
    Hash(AppDataHash),
    /// Validated full app data.
    Full(Arc<::app_data::ValidatedAppData>),
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

    pub fn flashloan(&self) -> Option<&app_data::Flashloan> {
        match self {
            Self::Hash(_) => None,
            Self::Full(data) => data.protocol.flashloan.as_ref(),
        }
    }

    pub fn wrappers(&self) -> &[app_data::WrapperCall] {
        match self {
            Self::Hash(_) => &[],
            Self::Full(data) => &data.protocol.wrappers,
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
        Self::Full(Arc::new(value))
    }
}

/// The length of the app data hash in bytes.
pub const APP_DATA_LEN: usize = 32;

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub struct AppDataHash(pub FixedBytes<APP_DATA_LEN>);

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
    #[error("failed to join task: {0}")]
    TaskJoinFailed(#[from] tokio::task::JoinError),
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
