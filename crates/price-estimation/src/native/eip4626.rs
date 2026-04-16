use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::PriceEstimationError,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    contracts::{ERC20, IERC4626},
    dashmap::DashSet,
    ethrpc::AlloyProvider,
    futures::{FutureExt, future::BoxFuture},
    num::{BigInt, BigRational, ToPrimitive},
    number::conversions::u256_to_big_rational,
    std::time::{Duration, Instant},
};

/// Estimates the native price of EIP-4626 vault tokens by:
/// 1. Querying `asset()` and `decimals()` on the vault
/// 2. Querying `convertToAssets(10^vault_decimals)` and `decimals()` on the
///    underlying asset
/// 3. Computing the conversion rate accounting for decimal differences
/// 4. Delegating to an inner estimator for the underlying token's native price
///
/// For non-vault tokens, delegates directly to the inner estimator
/// (pass-through).
///
/// Tokens whose `asset()` call reverts are remembered in a negative cache so
/// subsequent requests skip the RPC and go straight to the inner estimator.
pub struct Eip4626 {
    inner: Box<dyn NativePriceEstimating>,
    provider: AlloyProvider,
    /// Addresses that are known *not* to be EIP-4626 vaults (i.e. `asset()`
    /// reverted). Checked before making any RPC calls.
    non_vault_tokens: DashSet<Address>,
}

impl Eip4626 {
    pub fn new(
        inner: Box<dyn NativePriceEstimating>,
        provider: AlloyProvider,
        weth: Address,
    ) -> Self {
        Self {
            inner,
            provider,
            non_vault_tokens: {
                let non_vault_tokens = DashSet::new();
                non_vault_tokens.insert(weth);
                non_vault_tokens
            },
        }
    }

    async fn estimate(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        // Known non-vault or zero timeout: delegate directly. A zero timeout is
        // used by callers (e.g. the autopilot's WETH price fetch) as a
        // "best-effort / use cached data" signal — the inner estimator and its
        // callers treat it as advisory, not as a hard cutoff.
        if self.non_vault_tokens.contains(&token) {
            return self.inner.estimate_native_price(token, timeout).await;
        }

        let deadline = Instant::now() + timeout;
        let time_remaining = || deadline.saturating_duration_since(Instant::now());

        // Iteratively unwrap vault layers, accumulating the conversion rate.
        let mut current_token = token;
        let mut cumulative_rate = 1.0;

        while let Some((asset, rate)) = self
            .unwrap_vault_layer(current_token, time_remaining())
            .await?
        {
            cumulative_rate *= rate;
            current_token = asset;
        }

        let asset_price = self
            .inner
            .estimate_native_price(current_token, time_remaining())
            .await?;
        Ok(asset_price * cumulative_rate)
    }

    /// Returns `Ok(Some((asset, rate)))` if `token` is a vault, `Ok(None)` if
    /// it is known not to be a vault, or `Err` on a real RPC/computation
    /// failure.
    async fn unwrap_vault_layer(
        &self,
        token: Address,
        timeout: Duration,
    ) -> Result<Option<(Address, f64)>, PriceEstimationError> {
        if self.non_vault_tokens.contains(&token) {
            return Ok(None);
        }

        let result = tokio::time::timeout(timeout, self.calculate_conversion_rate(token))
            .await
            .map_err(|_| {
                PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "timeout exceeded during vault RPC calls for {token}"
                ))
            })?;

        match result {
            Ok(result) => Ok(Some(result)),
            // calculate_conversion_rate → fetch_vault_info adds the token to
            // non_vault_tokens when asset() reverts but decimals() succeeds.
            Err(_) if self.non_vault_tokens.contains(&token) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetches the vault's underlying asset address and decimals.
    /// On `asset()` revert the token is added to the negative cache if
    /// `decimals()` succeeded (i.e. it's a valid ERC-20 but not a vault).
    async fn fetch_vault_info(
        &self,
        token: Address,
    ) -> Result<(Address, u8), PriceEstimationError> {
        let vault = IERC4626::IERC4626::new(token, &self.provider);
        let vault_erc20 = ERC20::ERC20::new(token, &self.provider);
        let asset_call = vault.asset();
        let decimals_call = vault_erc20.decimals();
        let (asset_result, decimals_result) = tokio::join!(asset_call.call(), decimals_call.call());
        let asset = match asset_result {
            Ok(asset) => asset,
            Err(err) => {
                if decimals_result.is_ok() {
                    self.non_vault_tokens.insert(token);
                    metrics::non_vault_cache_size(self.non_vault_tokens.len());
                }
                return Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "failed to call asset() on {token}: {err}"
                )));
            }
        };
        // EIP-4626 vaults implement ERC-20, so decimals() must succeed if asset() did.
        let vault_decimals = decimals_result.map_err(|err| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call decimals() on {token}: {err}"
            ))
        })?;
        Ok((asset, vault_decimals))
    }

    /// Queries `convertToAssets(10^vault_decimals)` and the asset's decimals.
    async fn fetch_conversion_data(
        &self,
        token: Address,
        asset: Address,
        vault_decimals: u8,
    ) -> Result<(U256, u8), PriceEstimationError> {
        let one_token = U256::from(10u64)
            .checked_pow(U256::from(vault_decimals))
            .ok_or_else(|| {
                PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "vault decimals {vault_decimals} for {token} cause U256 overflow"
                ))
            })?;

        let vault = IERC4626::IERC4626::new(token, &self.provider);
        let asset_erc20 = ERC20::ERC20::new(asset, &self.provider);
        let convert_call = vault.convertToAssets(one_token);
        let asset_decimals_call = asset_erc20.decimals();
        tokio::try_join!(convert_call.call(), asset_decimals_call.call()).map_err(|err| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call convertToAssets()/decimals() on {token}: {err}"
            ))
        })
    }

    /// Fetches the underlying asset address and the shares-to-assets
    /// conversion rate from on-chain vault calls. On `asset()` revert the
    /// token is added to the negative cache. Transient errors (transport
    /// failures) are not cached.
    async fn calculate_conversion_rate(
        &self,
        token: Address,
    ) -> Result<(Address, f64), PriceEstimationError> {
        let (asset, vault_decimals) = self.fetch_vault_info(token).await?;
        let (assets, asset_decimals) = self
            .fetch_conversion_data(token, asset, vault_decimals)
            .await?;
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
    async fn non_vault_tokens_delegate_to_inner() {
        let mut inner = MockNativePriceEstimating::new();
        let token = Address::repeat_byte(0x42);
        let expected_price = 1.5;
        inner
            .expect_estimate_native_price()
            .withf(move |t, _| *t == token)
            .returning(move |_, _| Box::pin(async move { Ok(expected_price) }));

        let non_vault_tokens = DashSet::new();
        non_vault_tokens.insert(token);

        let estimator = Eip4626 {
            inner: Box::new(inner),
            // The provider is never reached because the cache short-circuits.
            provider: ethrpc::Web3::new_from_url("http://localhost:1").provider,
            non_vault_tokens,
        };

        let result = estimator
            .estimate(token, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap(), expected_price);
    }
}
