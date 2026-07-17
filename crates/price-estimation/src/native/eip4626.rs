//! Native price estimation for EIP-4626 vault tokens.
//!
//! The protocol's inner native price estimators only know about plain
//! ERC-20s, so this module wraps one and adds support for vaults: it asks
//! the vault for its underlying asset and conversion rate, prices the
//! underlying through the inner estimator, then rescales by the vault's
//! per-atom factor. Non-vault tokens pass through unchanged. See
//! [`Eip4626`] for the step-by-step derivation.
//!
//! # Terminology
//! - an *atom* is one U256 of a token — its smallest indivisible unit
//! - a *whole* token is `10^decimals` atoms (e.g. one whole USDC = `10^6`
//!   atoms)
//! - a *native price* is denominated in **wei per atom** of the priced token,
//!   where wei is one atom of the chain's native asset

use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::PriceEstimationError,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    contracts::{ERC20, IERC4626},
    dashmap::DashSet,
    ethrpc::{AlloyProvider, alloy::errors::ContractErrorExt},
    futures::{FutureExt, future::BoxFuture},
    model::order::BUY_ETH_ADDRESS,
    num::{BigInt, BigRational, ToPrimitive},
    number::conversions::u256_to_big_rational,
    std::time::{Duration, Instant},
};

/// Estimates the native price of EIP-4626 vault tokens.
///
/// To price one atom of a vault we need to know how many atoms of the
/// underlying that atom is worth, then multiply by the underlying's own
/// native price. The steps are:
///
/// 1. Call `asset()` and `decimals()` on the vault to identify the underlying
///    asset and the vault's whole-token size.
/// 2. Call `convertToAssets(10^vault_decimals)` to learn how many *atoms* of
///    the underlying *one whole* vault token is worth.
/// 3. Divide that result by `10^vault_decimals` to get the per-atom conversion
///    factor.
/// 4. Multiply by the underlying's native price (from the inner estimator) to
///    obtain the vault's native price.
///
/// Worked example — `ynUSDx`, an 18-decimal yield-bearing vault wrapping
/// USDC (6 decimals). The share price grows as yield accrues, so the rate is
/// a snapshot at query time; assume here that 1 whole ynUSDx is currently
/// worth 1.1 whole USDC at the queried block:
/// - `ynUSDx.asset() == USDC`, `ynUSDx.decimals() == 18`
/// - `convertToAssets(10^18) == 1_100_000` (= 1.1 whole USDC, in atoms)
/// - per-atom factor: `1_100_000 / 10^18 == 1.1e-12`
/// - if USDC's native price is `x`, then ynUSDx's is `x * 1.1e-12`
///
/// Tokens that don't classify as usable vaults are remembered in a negative
/// cache so subsequent requests skip the RPC and go straight to the inner
/// estimator.
pub struct Eip4626 {
    inner: Box<dyn NativePriceEstimating>,
    provider: AlloyProvider,
    /// Addresses that are known *not* to be (usable) EIP-4626 vaults. Checked
    /// before making any RPC calls.
    non_vault_tokens: DashSet<Address>,
}

impl Eip4626 {
    pub fn new(inner: Box<dyn NativePriceEstimating>, provider: AlloyProvider) -> Self {
        Self {
            inner,
            provider,
            // BUY_ETH_ADDRESS is not ERC-20, but it is a valid estimation address
            // so we need to make sure it bypasses the EIP-4626 estimator
            non_vault_tokens: DashSet::from_iter([BUY_ETH_ADDRESS]),
        }
    }

    async fn estimate(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        let deadline = Instant::now() + timeout;
        let (underlying, cumulative_rate) =
            tokio::time::timeout(timeout, self.unwrap_all_layers(token))
                .await
                .map_err(|_| {
                    PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                        "timeout while unwrapping EIP-4626 layers for {token}"
                    ))
                })??;

        let remaining = deadline.saturating_duration_since(Instant::now());
        let asset_price = self
            .inner
            .estimate_native_price(underlying, remaining)
            .await?;
        let estimate = asset_price * cumulative_rate;
        tracing::debug!(%token, estimate, "eip4626: estimated native price");
        Ok(estimate)
    }

    /// Follows the vault chain (e.g. vault → vault → asset) until reaching a
    /// non-vault token, returning the terminal token and the cumulative
    /// shares-to-assets rate.
    async fn unwrap_all_layers(
        &self,
        token: Address,
    ) -> Result<(Address, f64), PriceEstimationError> {
        let mut current_token = token;
        let mut cumulative_rate = 1.0;
        while let Some((asset, rate)) = self.unwrap_vault_layer(current_token).await? {
            cumulative_rate *= rate;
            current_token = asset;
        }
        Ok((current_token, cumulative_rate))
    }

    /// Returns:
    /// - `Ok(Some((asset, rate)))` when `token` is a vault.
    /// - `Ok(None)` when it's a plain ERC-20.
    /// - `Err` on RPC/computation failures that don't let us classify the
    ///   token.
    async fn unwrap_vault_layer(
        &self,
        token: Address,
    ) -> Result<Option<(Address, f64)>, PriceEstimationError> {
        if self.non_vault_tokens.contains(&token) {
            tracing::debug!(%token, "eip4626: cached non-vault, stop unwrapping");
            return Ok(None);
        }

        let Some((asset, vault_decimals)) = self.fetch_vault_info(token).await? else {
            self.mark_non_vault(token);
            tracing::debug!(%token, "eip4626: classified as non-vault");
            return Ok(None);
        };

        // Some tokens expose `asset()` yet revert here (e.g. a partial EIP-4626
        // implementation). Treat those as plain ERC-20s.
        let Some(assets) = self.fetch_conversion_data(token, vault_decimals).await? else {
            self.mark_non_vault(token);
            tracing::debug!(%token, "eip4626: convertToAssets() reverts, classified as non-vault");
            return Ok(None);
        };

        let rate = conversion_rate(assets, vault_decimals)
            .context("conversion rate is not representable as f64")
            .map_err(PriceEstimationError::EstimatorInternal)?;
        tracing::debug!(%token, %asset, rate, "eip4626: unwrapped vault layer");
        Ok(Some((asset, rate)))
    }

    /// Records `token` in the negative cache so subsequent requests skip the
    /// RPC probing and delegate straight to the inner estimator.
    fn mark_non_vault(&self, token: Address) {
        self.non_vault_tokens.insert(token);
        metrics::non_vault_cache_size(self.non_vault_tokens.len());
    }

    /// Fetches the vault's underlying asset address and vault token decimals.
    ///
    /// Returns:
    /// - `Ok(Some(...))` when `token` is a vault.
    /// - `Ok(None)` when `asset()` reverts (indicating it is a regular ERC-20).
    /// - `Err` on transient transport failures — those are *not* cached as
    ///   non-vault.
    async fn fetch_vault_info(
        &self,
        token: Address,
    ) -> Result<Option<(Address, u8)>, PriceEstimationError> {
        let vault = IERC4626::IERC4626::new(token, &self.provider);
        let vault_erc20 = ERC20::ERC20::new(token, &self.provider);
        let asset_call = vault.asset();
        let decimals_call = vault_erc20.decimals();
        let (asset_result, decimals_result) = tokio::join!(asset_call.call(), decimals_call.call());

        match asset_result {
            Ok(asset) => {
                // EIP-4626 vaults implement ERC-20 so decimals() must succeed too.
                let vault_decimals = decimals_result.map_err(|err| {
                    PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                        "failed to call decimals() on {token}: {err}"
                    ))
                })?;
                Ok(Some((asset, vault_decimals)))
            }
            // Contract-level revert on `asset()` + working `decimals()` =
            // plain ERC-20. Transient transport failures propagate so they
            // retry instead of pinning the token as non-vault.
            Err(err) if err.is_contract_revert() && decimals_result.is_ok() => Ok(None),
            Err(err) => Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call asset() on {token}: {err}"
            ))),
        }
    }

    /// Fetches `convertToAssets(10^vault_decimals)` — how many atomic units of
    /// the underlying asset correspond to one full vault token.
    ///
    /// Returns:
    /// - `Ok(Some(assets))` on success.
    /// - `Ok(None)` when `convertToAssets()` reverts. The caller must not treat
    ///   it as a vault.
    /// - `Err` on transient transport failures, so they retry instead of
    ///   pinning the token as non-vault.
    async fn fetch_conversion_data(
        &self,
        token: Address,
        vault_decimals: u8,
    ) -> Result<Option<U256>, PriceEstimationError> {
        let one_token = U256::from(10u64)
            .checked_pow(U256::from(vault_decimals))
            .ok_or_else(|| {
                PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                    "vault decimals {vault_decimals} for {token} cause U256 overflow"
                ))
            })?;

        let vault = IERC4626::IERC4626::new(token, &self.provider);
        match vault.convertToAssets(one_token).call().await {
            Ok(assets) => Ok(Some(assets)),
            Err(err) if err.is_contract_revert() => Ok(None),
            Err(err) => Err(PriceEstimationError::EstimatorInternal(anyhow::anyhow!(
                "failed to call convertToAssets() on {token}: {err}"
            ))),
        }
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

/// Converts the result of `convertToAssets(10^vault_decimals)` into the
/// per-atom factor (atoms of underlying per atom of vault) by dividing by
/// `10^vault_decimals`. See the module docstring for the full derivation.
///
/// This differs from the intuitive whole-to-whole share rate by
/// `10^(asset_decimals - vault_decimals)` — they only coincide when vault
/// and asset decimals match.
///
/// Returns `None` when the result is not representable as `f64`.
fn conversion_rate(assets: U256, vault_decimals: u8) -> Option<f64> {
    let denominator = BigRational::from_integer(BigInt::from(10u64).pow(vault_decimals as u32));
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
        alloy::{providers::mock::Asserter, sol_types::SolCall},
        std::borrow::Cow,
    };

    const TOLERANCE: f64 = 1e-9;

    /// Asserts `computed_rate` is within a `tolerance` of `expected_rate`.
    fn assert_rate_close(computed_rate: f64, expected_rate: f64, tolerance: f64) {
        let slack = expected_rate.abs() * tolerance;
        let expected_range = (expected_rate - slack)..(expected_rate + slack);
        assert!(
            expected_range.contains(&computed_rate),
            "computed_rate={computed_rate}, expected_rate={expected_rate}, tolerance={tolerance}",
        );
    }

    #[test]
    fn rate_math_same_decimals() {
        // 18-decimal vault wrapping 18-decimal asset:
        // 1 whole share = 1.5 whole asset tokens.
        // - vault.decimals() = 18, asset.decimals() = 18
        // - convertToAssets(10^18) = 1.5 * 10^18 (in asset atoms)
        // - per-atom factor = 1.5 * 10^18 / 10^18 = 1.5
        // - matches the whole-to-whole rate exactly when decimals align.
        let assets = U256::from(15u64) * U256::from(10u64).pow(U256::from(17u64));
        let computed_rate = conversion_rate(assets, 18).unwrap();

        assert_rate_close(computed_rate, 1.5, TOLERANCE);
    }

    #[test]
    fn rate_math_vault_18_asset_6() {
        // 18-decimal vault wrapping 6-decimal USDC:
        // 1 whole share = 1.5 whole USDC.
        // - vault.decimals() = 18, asset.decimals() = 6
        // - convertToAssets(10^18) = 1_500_000 (= 1.5 whole USDC, in atoms)
        // - per-atom factor = 1_500_000 / 10^18 = 1.5e-12
        // - = whole-share rate (1.5) scaled by 10^(asset_dec - vault_dec) = 10^-12.
        let assets = U256::from(1_500_000u64);
        let computed_rate = conversion_rate(assets, 18).unwrap();

        assert_rate_close(computed_rate, 1.5e-12, TOLERANCE);
    }

    #[test]
    fn rate_math_vault_6_asset_18() {
        // 6-decimal vault wrapping 18-decimal asset:
        // 1 whole share = 2 whole asset tokens.
        // - vault.decimals() = 6, asset.decimals() = 18
        // - convertToAssets(10^6) = 2 * 10^18 (= 2 whole tokens, in atoms)
        // - per-atom factor = 2 * 10^18 / 10^6 = 2e12
        // - = whole-share rate (2) scaled by 10^(asset_dec - vault_dec) = 10^12.
        let assets = U256::from(2u64) * U256::from(10u64).pow(U256::from(18u64));
        let computed_rate = conversion_rate(assets, 6).unwrap();

        assert_rate_close(computed_rate, 2e12, TOLERANCE);
    }

    /// Tests two (related) things:
    /// * Cached tokens bypass the EIP-4626 provider calls — i.e. calling
    ///   decimals, assets, etc
    /// * That the BUY_ETH_ADDRESS is cached by default (and the previous
    ///   applies to it)
    #[tokio::test]
    async fn buy_eth_address_bypasses_eth_calls() {
        let mut inner = MockNativePriceEstimating::new();
        let token = BUY_ETH_ADDRESS;
        let expected_price = 1.5;
        inner
            .expect_estimate_native_price()
            .withf(move |t, _| *t == token)
            .returning(move |_, _| Box::pin(async move { Ok(expected_price) }));

        let asserter = Asserter::new();
        asserter.push_failure_msg(Cow::from("calls are not being bypassed"));
        let web3 = ethrpc::Web3::with_asserter(asserter);

        let estimator = Eip4626::new(Box::new(inner), web3.provider);

        let result = estimator
            .estimate(token, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap(), expected_price);

        let result = estimator
            .estimate(Address::random(), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert!(
            matches!(result, Err(PriceEstimationError::EstimatorInternal(_))),
            "{result:?}"
        );
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

    #[tokio::test]
    async fn reverting_convert_to_assets_is_treated_as_non_vault() {
        let token = Address::repeat_byte(0x11);
        let underlying = Address::repeat_byte(0x22);
        let expected_price = 1.5;

        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .withf(move |t, _| *t == token)
            .returning(move |_, _| Box::pin(async move { Ok(expected_price) }));

        let asserter = Asserter::new();
        asserter.push_success(&IERC4626::IERC4626::assetCall::abi_encode_returns(
            &underlying,
        ));
        asserter.push_success(&ERC20::ERC20::decimalsCall::abi_encode_returns(&6u8));
        asserter.push_failure_msg("execution reverted");
        let web3 = ethrpc::Web3::with_asserter(asserter);

        let estimator = Eip4626::new(Box::new(inner), web3.provider);

        let result = estimator
            .estimate(token, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap(), expected_price);
        // The failed classification is cached so we don't re-probe on-chain.
        assert!(estimator.non_vault_tokens.contains(&token));
    }
}
