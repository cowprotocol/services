use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::PriceEstimationError,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    contracts::{ERC20, IERC4626},
    ethrpc::AlloyProvider,
    futures::{FutureExt, future::BoxFuture},
    num::{BigInt, BigRational, ToPrimitive},
    number::conversions::u256_to_big_rational,
    std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

/// Estimates the native price of EIP-4626 vault tokens by:
/// 1. Querying `asset()` and `decimals()` on the vault
/// 2. Querying `convertToAssets(10^vault_decimals)` and `decimals()` on the
///    underlying asset
/// 3. Computing the conversion rate accounting for decimal differences
/// 4. Delegating to an inner estimator for the underlying token's native price
///
/// Tokens that fail the `asset()` call are remembered in a negative cache so
/// subsequent requests skip the RPC entirely. Since most tokens are not
/// EIP-4626 vaults this avoids wasting a batched RPC round-trip per token per
/// estimation cycle. The cache is a `Mutex<HashSet<Address>>` (~2.4 MB at
/// 100k entries: 20-byte address + ~4 bytes overhead per entry) and is never
/// evicted — a process restart clears it, which also handles the edge case of
/// a proxy token upgrading to become a vault.
pub struct Eip4626 {
    inner: Arc<dyn NativePriceEstimating>,
    provider: AlloyProvider,
    /// Addresses that are known *not* to be EIP-4626 vaults (i.e. `asset()`
    /// reverted). Checked before making any RPC calls.
    non_vault_tokens: Mutex<HashSet<Address>>,
}

impl Eip4626 {
    pub fn new(inner: Arc<dyn NativePriceEstimating>, provider: AlloyProvider) -> Self {
        Self {
            inner,
            provider,
            non_vault_tokens: Mutex::new(HashSet::new()),
        }
    }

    /// Estimates the price of a vault token, if the token is not a vault token,
    /// an error is returned. The `timeout` budget is shared: vault RPC calls
    /// are individually bounded by `tokio::time::timeout`, and whatever time
    /// remains is forwarded to the inner estimator.
    async fn estimate(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        if self.non_vault_tokens.lock().unwrap().contains(&token) {
            return Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "{token} is not an EIP-4626 vault (cached)"
            )));
        }

        self.estimate_vault_token(token, timeout).await
    }

    /// Estimates the price of a *vault token*.
    async fn estimate_vault_token(
        &self,
        token: Address,
        timeout: Duration,
    ) -> NativePriceEstimateResult {
        let deadline = Instant::now() + timeout;

        let (asset, rate) = self.calculate_conversion_rate(token, timeout).await?;

        // Forward the remaining budget to the inner estimator so the total
        // wall-clock time stays within the caller's original timeout. This
        // matters for recursive Eip4626 chains where each layer spends time
        // on vault RPC calls.
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "timeout exceeded during vault RPC calls for {token}"
            )));
        }

        let asset_price = self.inner.estimate_native_price(asset, remaining).await?;

        Ok(asset_price * rate)
    }

    /// Fetches the underlying asset address and the shares-to-assets
    /// conversion rate from on-chain vault calls. On `asset()` failure the
    /// token is added to the negative cache.
    ///
    /// NOTE(jmg-duarte): `asset()` and `decimals()` are immutable for
    /// ERC-4626 vaults. Caching them in a positive-result map may be possible
    /// and useful to reduce network load.
    async fn calculate_conversion_rate(
        &self,
        token: Address,
        timeout: Duration,
    ) -> Result<(Address, f64), PriceEstimationError> {
        let deadline = Instant::now() + timeout;

        let vault = IERC4626::Instance::new(token, self.provider.clone());
        let vault_erc20 = ERC20::Instance::new(token, self.provider.clone());

        let asset_fut = vault.asset();
        let decimals_fut = vault_erc20.decimals();
        let (asset_result, decimals_result) = tokio::time::timeout(timeout, async {
            tokio::join!(asset_fut.call(), decimals_fut.call())
        })
        .await
        .map_err(|_| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "timeout during asset()/decimals() on {token}"
            ))
        })?;

        let asset: Address = match asset_result {
            Ok(addr) => addr,
            Err(e) => {
                {
                    let mut cache = self.non_vault_tokens.lock().unwrap();
                    cache.insert(token);
                    metrics::non_vault_cache_size(cache.len());
                }
                return Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "failed to call asset() on {token}: {e}"
                )));
            }
        };

        let vault_decimals: u8 = decimals_result.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call decimals() on {token}: {e}"
            ))
        })?;

        let one_token = U256::from(10u64).pow(U256::from(vault_decimals));
        let asset_erc20 = ERC20::Instance::new(asset, self.provider.clone());
        let convert_fut = vault.convertToAssets(one_token);
        let asset_decimals_fut = asset_erc20.decimals();
        let remaining = deadline.saturating_duration_since(Instant::now());
        let (convert_result, asset_decimals_result) = tokio::time::timeout(remaining, async {
            tokio::join!(convert_fut.call(), asset_decimals_fut.call())
        })
        .await
        .map_err(|_| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "timeout during convertToAssets()/asset decimals() on {token}"
            ))
        })?;

        let assets: U256 = convert_result.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call convertToAssets() on {token}: {e}"
            ))
        })?;

        let asset_decimals: u8 = asset_decimals_result.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call decimals() on underlying asset {asset}: {e}"
            ))
        })?;

        let rate = conversion_rate(assets, asset_decimals)
            .context("conversion rate is not representable as f64")
            .map_err(PriceEstimationError::EstimatorInternal)?;

        Ok((asset, rate))
    }
}

impl NativePriceEstimating for Eip4626 {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        self.estimate(token, timeout).boxed()
    }
}

/// Computes the full-asset-tokens per full-vault-token conversion rate.
///
/// `assets` is the return value of `convertToAssets(10^vault_decimals)` — i.e.
/// asset-atomic-units for exactly 1 full vault token. Dividing by
/// `10^asset_decimals` converts to full asset tokens.
///
/// Returns `None` when the result is not representable as `f64`.
fn conversion_rate(assets: U256, asset_decimals: u8) -> Option<f64> {
    let denominator = BigRational::from_integer(BigInt::from(10u64).pow(asset_decimals as u32));
    (u256_to_big_rational(&assets) / denominator).to_f64()
}

mod metrics {
    use {observe::metrics, prometheus::IntGauge};

    #[derive(prometheus_metric_storage::MetricStorage)]
    struct Metrics {
        /// Number of tokens in the EIP-4626 negative cache (known non-vault
        /// tokens).
        eip4626_non_vault_cache_size: IntGauge,
    }

    impl Metrics {
        fn get() -> &'static Self {
            Metrics::instance(metrics::get_storage_registry()).unwrap()
        }
    }

    pub(super) fn non_vault_cache_size(size: usize) {
        Metrics::get()
            .eip4626_non_vault_cache_size
            .set(i64::try_from(size).unwrap_or(i64::MAX));
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{HEALTHY_PRICE_ESTIMATION_TIME, native::MockNativePriceEstimating},
    };

    #[test]
    fn rate_math_same_decimals() {
        // 18-decimal vault wrapping 18-decimal asset, 1 share = 1.5 asset tokens.
        // convertToAssets(10^18) = 1.5 * 10^18 asset-atomic-units
        let assets = U256::from(15u64) * U256::from(10u64).pow(U256::from(17u64));
        let rate = conversion_rate(assets, 18).unwrap();
        assert!((rate - 1.5).abs() < 1e-9, "rate={rate}");
    }

    #[test]
    fn rate_math_vault_18_asset_6() {
        // 18-decimal vault wrapping 6-decimal USDC, 1 share = 1.5 USDC.
        // convertToAssets(10^18) = 1_500_000 asset-atomic-units (1.5 * 10^6)
        let assets = U256::from(1_500_000u64);
        let rate = conversion_rate(assets, 6).unwrap();
        assert!((rate - 1.5).abs() < 1e-9, "rate={rate}");
    }

    #[test]
    fn rate_math_vault_6_asset_18() {
        // 6-decimal vault wrapping 18-decimal asset, 1 share = 2 asset tokens.
        // convertToAssets(10^6) = 2 * 10^18 asset-atomic-units
        let assets = U256::from(2u64) * U256::from(10u64).pow(U256::from(18u64));
        let rate = conversion_rate(assets, 18).unwrap();
        assert!((rate - 2.0).abs() < 1e-9, "rate={rate}");
    }

    #[tokio::test]
    async fn non_vault_tokens_are_cached() {
        let inner = MockNativePriceEstimating::new();
        let non_vault_tokens = Mutex::new(HashSet::new());
        let token = Address::repeat_byte(0x42);

        // Pre-populate the negative cache.
        non_vault_tokens.lock().unwrap().insert(token);

        let estimator = Eip4626 {
            inner: Arc::new(inner),
            // The provider is never reached because the cache short-circuits.
            provider: ethrpc::Web3::new_from_url("http://localhost:1").provider,
            non_vault_tokens,
        };

        // The estimate should fail immediately without making any RPC calls
        // (the mock inner has no expectations set, so any call would panic).
        let result = estimator
            .estimate(token, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not an EIP-4626 vault (cached)"), "{err}");
    }
}
