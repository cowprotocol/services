use {
    anyhow::{Context, Result},
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    number::serialization::HexOrDecimalU256,
    primitive_types::U256,
    reqwest::Url,
    serde::Deserialize,
    serde_with::serde_as,
    std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    tracing::instrument,
};

/// A gas price estimator that fetches gas prices from a driver instance with
/// caching.
#[derive(Clone)]
pub struct DriverGasEstimator {
    client: reqwest::Client,
    url: Url,
    cache: Arc<Mutex<Option<CachedGasPrice>>>,
    cache_duration: Duration,
}

#[derive(Debug, Clone)]
struct CachedGasPrice {
    price: GasPrice1559,
    timestamp: Instant,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GasPriceResponse {
    /// The maximum fee per gas (maxFeePerGas in EIP-1559)
    #[serde_as(as = "HexOrDecimalU256")]
    max_fee_per_gas: U256,
    /// The maximum priority fee per gas (maxPriorityFeePerGas/tip in EIP-1559)
    #[serde_as(as = "HexOrDecimalU256")]
    max_priority_fee_per_gas: U256,
    /// The current base fee per gas
    #[serde_as(as = "HexOrDecimalU256")]
    base_fee_per_gas: U256,
}

impl DriverGasEstimator {
    pub async fn new(client: reqwest::Client, driver_url: Url) -> Result<Self> {
        let instance = Self {
            client,
            url: driver_url,
            cache: Arc::new(Mutex::new(None)),
            // Cache for 5 seconds to avoid too many requests but still be responsive
            cache_duration: Duration::from_secs(5),
        };

        //test connection
        instance.estimate().await?;

        Ok(instance)
    }

    #[instrument(skip(self))]
    async fn fetch_gas_price(&self) -> Result<GasPrice1559> {
        let response = self
            .client
            .get(self.url.clone())
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("failed to send request to driver")?
            .error_for_status()
            .context("driver returned error status")?
            .json::<GasPriceResponse>()
            .await
            .context("failed to parse driver response")?;

        // The driver returns all three gas price components
        Ok(GasPrice1559 {
            base_fee_per_gas: response.base_fee_per_gas.to_f64_lossy(),
            max_fee_per_gas: response.max_fee_per_gas.to_f64_lossy(),
            max_priority_fee_per_gas: response.max_priority_fee_per_gas.to_f64_lossy(),
        })
    }

    async fn estimate(&self) -> Result<GasPrice1559> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.as_ref() {
                if cached.timestamp.elapsed() < self.cache_duration {
                    tracing::debug!(gasPrice = ?cached.price, "returning cached gas price");
                    return Ok(cached.price);
                }
            }
        }

        // Cache miss or expired, fetch new price
        let price = self.fetch_gas_price().await?;
        tracing::debug!(gasPrice = ?price, "fetched fresh gas price from driver");

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            *cache = Some(CachedGasPrice {
                price,
                timestamp: Instant::now(),
            });
        }

        Ok(price)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for DriverGasEstimator {
    #[instrument(skip(self))]
    async fn estimate_with_limits(
        &self,
        _gas_limit: f64,
        _time_limit: Duration,
    ) -> Result<GasPrice1559> {
        self.estimate().await
    }
}
