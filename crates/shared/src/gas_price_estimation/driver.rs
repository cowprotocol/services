use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559},
    alloy::primitives::U256,
    anyhow::{Context, Result},
    number::serialization::HexOrDecimalU256,
    reqwest::Url,
    serde::Deserialize,
    serde_with::serde_as,
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
    price: GasPrice1559,
    timestamp: Instant,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Gas price components in EIP-1559 format.
struct GasPriceResponse {
    #[serde_as(as = "HexOrDecimalU256")]
    max_fee_per_gas: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    max_priority_fee_per_gas: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    base_fee_per_gas: U256,
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
    async fn fetch_gas_price(&self) -> Result<GasPrice1559> {
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

        Ok(GasPrice1559 {
            base_fee_per_gas: f64::from(response.base_fee_per_gas),
            max_fee_per_gas: f64::from(response.max_fee_per_gas),
            max_priority_fee_per_gas: f64::from(response.max_priority_fee_per_gas),
        })
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for DriverGasEstimator {
    #[instrument(skip(self))]
    async fn estimate(&self) -> Result<GasPrice1559> {
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
