use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::PriceEstimationError,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    contracts::{ERC20, IERC4626},
    ethrpc::AlloyProvider,
    futures::{FutureExt, future::BoxFuture},
    num::ToPrimitive,
    number::conversions::u256_to_big_rational,
    std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

/// Estimates the native price of EIP-4626 vault tokens by:
/// 1. Calling `asset()` and `decimals()` in parallel
/// 2. Calling `convertToAssets(10^decimals)` to find the conversion rate
/// 3. Delegating to an inner estimator for the underlying token's native price
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
        let erc20 = ERC20::Instance::new(token, self.provider.clone());

        // Parallel calls get batched into a single RPC request by alloy.
        let asset_builder = vault.asset();
        let decimals_builder = erc20.decimals();
        let (asset_result, decimals_result) = tokio::time::timeout(timeout, async {
            tokio::join!(asset_builder.call(), decimals_builder.call())
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
                self.non_vault_tokens.lock().unwrap().insert(token);
                return Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "failed to call asset() on {token}: {e}"
                )));
            }
        };

        let decimals: u8 = decimals_result.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call decimals() on {token}: {e}"
            ))
        })?;

        let shares = U256::from(10u64).pow(U256::from(decimals));

        let remaining = deadline.saturating_duration_since(Instant::now());
        let assets: U256 = tokio::time::timeout(remaining, vault.convertToAssets(shares).call())
            .await
            .map_err(|_| {
                PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "timeout during convertToAssets() on {token}"
                ))
            })?
            .map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "failed to call convertToAssets() on {token}: {e}"
                ))
            })?;

        let rate = (u256_to_big_rational(&assets) / u256_to_big_rational(&shares))
            .to_f64()
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{HEALTHY_PRICE_ESTIMATION_TIME, native::MockNativePriceEstimating},
    };

    #[test]
    fn rate_math() {
        // 6-decimal vault where 1 share = 1.5 underlying tokens
        let decimals = 6u8;
        let shares = U256::from(10u64).pow(U256::from(decimals));
        let assets = U256::from(1_500_000u64); // 1.5 * 10^6
        let rate = (u256_to_big_rational(&assets) / u256_to_big_rational(&shares))
            .to_f64()
            .unwrap();
        assert!((rate - 1.5).abs() < 1e-9);
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
