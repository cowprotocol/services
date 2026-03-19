use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::PriceEstimationError,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    contracts::alloy::{ERC20, IERC4626},
    ethrpc::AlloyProvider,
    futures::{FutureExt, future::BoxFuture},
    num::ToPrimitive,
    number::conversions::u256_to_big_rational,
    std::{sync::Arc, time::Duration},
};

/// Estimates the native price of EIP-4626 vault tokens by:
/// 1. Calling `asset()` to find the underlying token
/// 2. Calling `decimals()` to determine the vault's precision
/// 3. Calling `convertToAssets(10^decimals)` to find the conversion rate
/// 4. Delegating to an inner estimator for the underlying token's native price
pub struct Eip4626 {
    inner: Arc<dyn NativePriceEstimating>,
    provider: AlloyProvider,
}

impl Eip4626 {
    pub fn new(inner: Arc<dyn NativePriceEstimating>, provider: AlloyProvider) -> Self {
        Self { inner, provider }
    }

    async fn estimate(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        let vault = IERC4626::Instance::new(token, self.provider.clone());
        let erc20 = ERC20::Instance::new(token, self.provider.clone());

        let asset: Address = vault.asset().call().await.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call asset() on {token}: {e}"
            ))
        })?;

        let decimals: u8 = erc20.decimals().call().await.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call decimals() on {token}: {e}"
            ))
        })?;

        let shares = U256::from(10u64).pow(U256::from(decimals));

        let assets: U256 = vault.convertToAssets(shares).call().await.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call convertToAssets() on {token}: {e}"
            ))
        })?;

        let asset_price = self.inner.estimate_native_price(asset, timeout).await?;

        let rate = (u256_to_big_rational(&assets) / u256_to_big_rational(&shares))
            .to_f64()
            .context("conversion rate is not representable as f64")
            .map_err(PriceEstimationError::EstimatorInternal)?;

        Ok(asset_price * rate)
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

    /// Requires a live node; run with:
    ///   NODE_URL=... cargo test -p price-estimation -- eip4626 --ignored
    /// --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_sdai() {
        // sDAI on mainnet: vault wrapping DAI
        let sdai = alloy::primitives::address!("83F20F44975D03b1b09e64809B757c47f942BEeA");

        let web3 = ethrpc::Web3::new_from_env();

        let mut inner = MockNativePriceEstimating::new();
        inner.expect_estimate_native_price().returning(|token, _| {
            let dai = alloy::primitives::address!("6B175474E89094C44Da98b954EedeAC495271d0F");
            assert_eq!(token, dai, "should price the underlying DAI, not sDAI");
            async { Ok(3.3e-4_f64) }.boxed()
        });

        let estimator = Eip4626::new(Arc::new(inner), web3.provider);
        let price = estimator
            .estimate_native_price(sdai, HEALTHY_PRICE_ESTIMATION_TIME)
            .await
            .unwrap();

        // sDAI should be worth slightly more than DAI due to accrued interest
        println!("sDAI native price: {price}");
        assert!(price > 3.3e-4_f64 * 0.99 && price < 3.3e-4_f64 * 1.20);
    }
}
