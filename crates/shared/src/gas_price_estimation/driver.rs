use {
    crate::gas_price_estimation::GasPriceEstimating,
    alloy::eips::eip1559::Eip1559Estimation,
    anyhow::{Context, Result},
    reqwest::Url,
    serde::Deserialize,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Mutex,
    tracing::instrument,
};

/// A gas price estimator that fetches gas prices from a driver instance with
/// caching.
#[derive(Clone)]
pub struct DriverGasEstimator {
    client: reqwest::Client,
    url: Url,
    cache: Arc<Mutex<Option<CachedGasPrice>>>,
}

#[derive(Debug, Clone)]
struct CachedGasPrice {
    price: Eip1559Estimation,
    timestamp: Instant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Gas price components in EIP-1559 format.
struct GasPriceResponse {
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
}

const CACHE_DURATION: Duration = Duration::from_secs(5);

impl DriverGasEstimator {
    pub fn new(client: reqwest::Client, driver_url: Url) -> Self {
        Self {
            client,
            url: driver_url,
            cache: Arc::new(Mutex::new(None)),
        }
    }

    #[instrument(skip(self))]
    async fn fetch_gas_price(&self) -> Result<Eip1559Estimation> {
        let response = self
            .client
            .get(self.url.clone())
            .send()
            .await
            .context("failed to send request to driver")?
            .error_for_status()
            .context("driver returned error status")?
            .json::<GasPriceResponse>()
            .await
            .context("failed to parse driver response")?;

        Ok(Eip1559Estimation {
            max_fee_per_gas: response.max_fee_per_gas,
            max_priority_fee_per_gas: response.max_priority_fee_per_gas,
        })
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for DriverGasEstimator {
    #[instrument(skip(self))]
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        // Lock cache for entire duration of this method to prevent concurrent network
        // requests
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref()
            && cached.timestamp.elapsed() < CACHE_DURATION
        {
            return Ok(cached.price);
        }

        // Cache miss or expired, fetch new price and update cache
        let price = self.fetch_gas_price().await?;

        *cache = Some(CachedGasPrice {
            price,
            timestamp: Instant::now(),
        });

        Ok(price)
    }
}
