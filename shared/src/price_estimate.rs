use crate::{
    bad_token::BadTokenDetecting,
    baseline_solver::{
        estimate_buy_amount, estimate_sell_amount, path_candidates, token_path_to_pair_path,
        Estimate, DEFAULT_MAX_HOPS,
    },
    conversions::U256Ext,
    recent_block_cache::Block,
    sources::uniswap::pool_fetching::{Pool, PoolFetching},
};
use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use ethcontract::{H160, U256};
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use model::{order::OrderKind, TokenPair};
use num::{BigRational, ToPrimitive};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PriceEstimationError {
    // Represents a failure when no liquidity between sell and buy token via the native token can be found
    #[error("Token {0:?} not supported")]
    UnsupportedToken(H160),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait PriceEstimating: Send + Sync {
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in buy_token.
    /// The resulting price is how many units of sell_token needs to be sold for one unit of
    /// buy_token (sell_amount / buy_amount).
    /// amount must be nonzero.
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational, PriceEstimationError>;

    /// Returns the expected gas cost for this given trade
    /// amount must be nonzero.
    async fn estimate_gas(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<U256, PriceEstimationError>;

    async fn estimate_price_as_f64(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<f64, PriceEstimationError> {
        self.estimate_price(sell_token, buy_token, amount, kind)
            .await
            .and_then(|price| {
                price
                    .to_f64()
                    .ok_or_else(|| anyhow!("Cannot convert price ratio to float").into())
            })
    }

    fn amount_to_estimate_prices_with(&self) -> U256;

    // Returns a vector of (rational) prices for the given tokens denominated
    // in denominator_token or an error in case there is an error computing any
    // of the prices in the vector.
    async fn estimate_prices(
        &self,
        tokens: &[H160],
        denominator_token: H160,
    ) -> Vec<Result<BigRational, PriceEstimationError>> {
        join_all(tokens.iter().map(|token| async move {
            if *token != denominator_token {
                self.estimate_price(
                    denominator_token,
                    *token,
                    self.amount_to_estimate_prices_with(),
                    OrderKind::Sell,
                )
                .await
            } else {
                Ok(num::one())
            }
        }))
        .await
    }
}

pub struct BaselinePriceEstimator {
    pool_fetcher: Arc<dyn PoolFetching>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    base_tokens: HashSet<H160>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_token: H160,
    amount_to_estimate_prices_with: U256,
}

impl BaselinePriceEstimator {
    pub fn new(
        pool_fetcher: Arc<dyn PoolFetching>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        base_tokens: HashSet<H160>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        native_token: H160,
        amount_to_estimate_prices_with: U256,
    ) -> Self {
        Self {
            pool_fetcher,
            gas_estimator,
            base_tokens,
            bad_token_detector,
            native_token,
            amount_to_estimate_prices_with,
        }
    }

    async fn ensure_token_supported(&self, token: H160) -> Result<(), PriceEstimationError> {
        match self.bad_token_detector.detect(token).await {
            Ok(quality) => {
                if quality.is_good() {
                    Ok(())
                } else {
                    Err(PriceEstimationError::UnsupportedToken(token))
                }
            }
            Err(err) => Err(PriceEstimationError::Other(err)),
        }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for BaselinePriceEstimator {
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational, PriceEstimationError> {
        let price = self
            .estimate_price_helper(sell_token, buy_token, amount, kind, true)
            .await?
            .1;
        Ok(price)
    }

    async fn estimate_gas(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<U256, PriceEstimationError> {
        let path = self
            .estimate_price_helper(sell_token, buy_token, amount, kind, true)
            .await?
            .0;
        let trades = path.len() - 1;
        // This could be more accurate by actually simulating the settlement (since different tokens might have more or less expensive transfer costs)
        // For the standard OZ token the cost is roughly 110k for a direct trade, 170k for a 1 hop trade, 230k for a 2 hop trade.
        const BASELINE_GAS_PER_HOP: u64 = 60_000;
        const BASELINE_FIXED_OVERHEAD: u64 = 50_000;
        // Extrapolated from gp-v2-contract's `yarn bench:uniswap`
        const GP_OVERHEAD: u64 = 90_000;
        return Ok(U256::from(GP_OVERHEAD)
            + BASELINE_FIXED_OVERHEAD
            + BASELINE_GAS_PER_HOP * trades as u64);
    }

    fn amount_to_estimate_prices_with(&self) -> U256 {
        self.amount_to_estimate_prices_with
    }
}

impl BaselinePriceEstimator {
    /// Returns the path and the price.
    async fn estimate_price_helper(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        consider_gas_costs: bool,
    ) -> Result<(Vec<H160>, BigRational), PriceEstimationError> {
        self.ensure_token_supported(sell_token).await?;
        self.ensure_token_supported(buy_token).await?;
        if sell_token == buy_token {
            return Ok((Vec::new(), num::one()));
        }
        if amount.is_zero() {
            return Err(anyhow!("Attempt to estimate price of a trade with zero amount.").into());
        }
        let gas_price = self.gas_estimator.estimate().await?;
        match kind {
            OrderKind::Buy => {
                let (path, sell_amount) = self
                    .best_execution_buy_order(
                        sell_token,
                        buy_token,
                        amount,
                        gas_price,
                        consider_gas_costs,
                    )
                    .await?;
                Ok((
                    path,
                    BigRational::new(sell_amount.to_big_int(), amount.to_big_int()),
                ))
            }
            OrderKind::Sell => {
                let (path, buy_amount) = self
                    .best_execution_sell_order(
                        sell_token,
                        buy_token,
                        amount,
                        gas_price,
                        consider_gas_costs,
                    )
                    .await?;
                // Due to integer rounding it is possible that we find path but the amount is 0.
                if buy_amount.is_zero() {
                    return Err(anyhow!("infinite price").into());
                }
                Ok((
                    path,
                    BigRational::new(amount.to_big_int(), buy_amount.to_big_int()),
                ))
            }
        }
    }

    fn evaluate_path_of_sell_order_disregarding_gas_costs(
        &self,
        buy_estimate: Estimate<U256, Pool>,
    ) -> BigRational {
        buy_estimate.value.to_big_rational()
    }
    fn evaluate_path_of_sell_order_considering_gas_costs(
        &self,
        buy_token_price_in_native_token: BigRational,
        gas_price: f64,
        buy_estimate: Estimate<U256, Pool>,
    ) -> BigRational {
        let buy_amount_in_native_token =
            buy_estimate.value.to_big_rational() * buy_token_price_in_native_token;
        let tx_cost_in_native_token = U256::from_f64_lossy(gas_price).to_big_rational()
            * BigRational::from_integer(buy_estimate.gas_cost().into());
        buy_amount_in_native_token - tx_cost_in_native_token
    }

    #[async_recursion]
    async fn best_execution_sell_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        sell_amount: U256,
        gas_price: f64,
        consider_gas_costs: bool,
    ) -> Result<(Vec<H160>, U256)> {
        let path_comparison = if consider_gas_costs {
            // Do not consider gas costs below to avoid infinite recursion.
            let buy_token_price_in_native_token = self
                .estimate_price_helper(
                    self.native_token,
                    buy_token,
                    self.amount_to_estimate_prices_with,
                    OrderKind::Sell,
                    false,
                )
                .await?
                .1;
            Box::new(move |buy_estimate: Estimate<U256, Pool>| {
                self.evaluate_path_of_sell_order_considering_gas_costs(
                    buy_token_price_in_native_token.clone(),
                    gas_price,
                    buy_estimate,
                )
            }) as Box<dyn Fn(Estimate<U256, Pool>) -> BigRational + Send + Sync>
        } else {
            Box::new(|buy_estimate: Estimate<U256, Pool>| {
                self.evaluate_path_of_sell_order_disregarding_gas_costs(buy_estimate)
            }) as Box<dyn Fn(Estimate<U256, Pool>) -> BigRational + Send + Sync>
        };

        self.best_execution(
            sell_token,
            buy_token,
            sell_amount,
            |amount, path, pools| {
                estimate_buy_amount(amount, path, pools)
                    .map(&path_comparison)
                    .unwrap_or_else(|| -U256::max_value().to_big_rational())
            },
            |amount, path, pools| {
                estimate_buy_amount(amount, path, pools).map(|estimate| estimate.value)
            },
        )
        .await
    }

    fn evaluate_path_of_buy_order_disregarding_gas_costs(
        &self,
        sell_estimate: Estimate<U256, Pool>,
    ) -> BigRational {
        -sell_estimate.value.to_big_rational()
    }
    fn evaluate_path_of_buy_order_considering_gas_costs(
        &self,
        sell_token_price_in_native_token: BigRational,
        gas_price: f64,
        sell_estimate: Estimate<U256, Pool>,
    ) -> BigRational {
        let sell_amount_in_native_token =
            sell_estimate.value.to_big_rational() * sell_token_price_in_native_token;
        let tx_cost_in_native_token = U256::from_f64_lossy(gas_price).to_big_rational()
            * BigRational::from_integer(sell_estimate.gas_cost().into());
        -sell_amount_in_native_token - tx_cost_in_native_token
    }

    #[async_recursion]
    async fn best_execution_buy_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        buy_amount: U256,
        gas_price: f64,
        consider_gas_costs: bool,
    ) -> Result<(Vec<H160>, U256)> {
        let path_comparison = if consider_gas_costs {
            // Do not consider gas costs below to avoid infinite recursion.
            let sell_token_price_in_native_token = self
                .estimate_price_helper(
                    self.native_token,
                    sell_token,
                    self.amount_to_estimate_prices_with,
                    OrderKind::Sell,
                    false,
                )
                .await?
                .1;
            Box::new(move |sell_estimate: Estimate<U256, Pool>| {
                self.evaluate_path_of_buy_order_considering_gas_costs(
                    sell_token_price_in_native_token.clone(),
                    gas_price,
                    sell_estimate,
                )
            }) as Box<dyn Fn(Estimate<U256, Pool>) -> BigRational + Send + Sync>
        } else {
            Box::new(|sell_estimate: Estimate<U256, Pool>| {
                self.evaluate_path_of_buy_order_disregarding_gas_costs(sell_estimate)
            }) as Box<dyn Fn(Estimate<U256, Pool>) -> BigRational + Send + Sync>
        };

        self.best_execution(
            sell_token,
            buy_token,
            buy_amount,
            |amount, path, pools| {
                estimate_sell_amount(amount, path, pools)
                    .map(&path_comparison)
                    .unwrap_or_else(|| -U256::max_value().to_big_rational())
            },
            |amount, path, pools| {
                estimate_sell_amount(amount, path, pools).map(|estimate| estimate.value)
            },
        )
        .await
    }

    async fn best_execution<AmountFn, CompareFn, O, Amount>(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        comparison: CompareFn,
        resulting_amount: AmountFn,
    ) -> Result<(Vec<H160>, Amount)>
    where
        AmountFn: Fn(U256, &[H160], &HashMap<TokenPair, Vec<Pool>>) -> Option<Amount>,
        CompareFn: Fn(U256, &[H160], &HashMap<TokenPair, Vec<Pool>>) -> O,
        O: Ord,
    {
        let path_candidates =
            path_candidates(sell_token, buy_token, &self.base_tokens, DEFAULT_MAX_HOPS);
        let all_pairs = path_candidates
            .iter()
            .flat_map(|candidate| token_path_to_pair_path(candidate).into_iter())
            .collect();
        let pools = self
            .pool_fetcher
            .fetch(all_pairs, Block::Recent)
            .await?
            .into_iter()
            .fold(HashMap::<_, Vec<Pool>>::new(), |mut pools, pool| {
                pools.entry(pool.tokens).or_default().push(pool);
                pools
            });
        let best_path = path_candidates
            .iter()
            .max_by_key(|path| comparison(amount, path, &pools))
            .ok_or(anyhow!(format!(
                "No Uniswap path found between {:#x} and {:#x}",
                sell_token, buy_token
            )))?;
        Ok((
            best_path.clone(),
            resulting_amount(amount, best_path, &pools).ok_or_else(|| {
                anyhow!(format!(
                    "No valid path found between {:#x} and {:#x}",
                    sell_token, buy_token
                ))
            })?,
        ))
    }
}

pub mod mocks {
    use super::*;

    pub struct FakePriceEstimator(pub BigRational);
    #[async_trait::async_trait]
    impl PriceEstimating for FakePriceEstimator {
        async fn estimate_price(
            &self,
            _: H160,
            _: H160,
            _: U256,
            _: OrderKind,
        ) -> Result<BigRational, PriceEstimationError> {
            Ok(self.0.clone())
        }

        async fn estimate_gas(
            &self,
            _: H160,
            _: H160,
            _: U256,
            _: OrderKind,
        ) -> Result<U256, PriceEstimationError> {
            Ok(100_000.into())
        }

        fn amount_to_estimate_prices_with(&self) -> U256 {
            1.into()
        }
    }

    pub struct FailingPriceEstimator();
    #[async_trait::async_trait]
    impl PriceEstimating for FailingPriceEstimator {
        async fn estimate_price(
            &self,
            _: H160,
            _: H160,
            _: U256,
            _: OrderKind,
        ) -> Result<BigRational, PriceEstimationError> {
            Err(anyhow!("error").into())
        }

        async fn estimate_gas(
            &self,
            _: H160,
            _: H160,
            _: U256,
            _: OrderKind,
        ) -> Result<U256, PriceEstimationError> {
            Err(anyhow!("error").into())
        }

        fn amount_to_estimate_prices_with(&self) -> U256 {
            1.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{bad_token::list_based::ListBasedDetector, baseline_solver::BaselineSolvable};
    use assert_approx_eq::assert_approx_eq;
    use maplit::hashset;
    use std::collections::HashSet;
    use std::sync::Mutex;

    use super::*;
    use crate::{
        gas_price_estimation::FakeGasPriceEstimator,
        sources::uniswap::pool_fetching::{Pool, PoolFetching},
    };

    struct FakePoolFetcher(Vec<Pool>);
    #[async_trait::async_trait]
    impl PoolFetching for FakePoolFetcher {
        async fn fetch(&self, token_pairs: HashSet<TokenPair>, _: Block) -> Result<Vec<Pool>> {
            Ok(self
                .0
                .clone()
                .into_iter()
                .filter(|pool| token_pairs.contains(&pool.tokens))
                .collect())
        }
    }

    #[tokio::test]
    async fn estimate_price_on_direct_pair() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(28), 10u128.pow(27)),
        );

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_b,
            1.into(),
        );

        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_a, token_a, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            1.0
        );
        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_a, token_a, U256::exp10(18), OrderKind::Sell)
                .await
                .unwrap(),
            1.0
        );
        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_a, token_b, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            10.03,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_a, token_b, U256::exp10(18), OrderKind::Sell)
                .await
                .unwrap(),
            10.03,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_b, token_a, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            0.1003,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price_as_f64(token_b, token_a, U256::exp10(18), OrderKind::Sell)
                .await
                .unwrap(),
            0.1003,
            1.0e-4
        );
    }

    #[tokio::test]
    async fn return_error_if_no_token_found() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_fetcher = Arc::new(FakePoolFetcher(vec![]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_a,
            1.into(),
        );

        assert!(estimator
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Buy)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn return_error_if_token_denied() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_ab = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(28), 10u128.pow(27)),
        );
        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool_ab]));
        let bad_token = Arc::new(ListBasedDetector::deny_list(vec![token_a]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            bad_token,
            token_a,
            1.into(),
        );

        let result = estimator
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Buy)
            .await
            .unwrap_err();
        assert_eq!(
            format!("Token {:?} not supported", token_a),
            result.to_string()
        );
        let result = estimator
            .estimate_price(token_b, token_a, 1.into(), OrderKind::Buy)
            .await
            .unwrap_err();
        assert_eq!(
            format!("Token {:?} not supported", token_a),
            result.to_string()
        );
    }

    #[tokio::test]
    async fn return_error_if_invalid_reserves() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (0, 10));

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_a,
            1.into(),
        );

        assert!(estimator
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Buy)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn price_estimate_containing_valid_and_invalid_paths() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);

        // The path via the base token does not exist (making it an invalid path)
        let base_token = H160::from_low_u64_be(3);

        let pool = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(28), 10u128.pow(27)),
        );

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(base_token),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_b,
            1.into(),
        );

        assert!(estimator
            .estimate_price(token_a, token_b, 100.into(), OrderKind::Sell)
            .await
            .is_ok());
        assert!(estimator
            .estimate_price(token_a, token_b, 100.into(), OrderKind::Buy)
            .await
            .is_ok());
    }

    fn pool_price(
        pool: &Pool,
        token_out: H160,
        amount_in: impl Into<U256>,
        token_in: H160,
    ) -> BigRational {
        let amount_in = amount_in.into();
        BigRational::new(
            amount_in.to_big_int(),
            pool.get_amount_out(token_out, (amount_in, token_in))
                .unwrap()
                .as_u128()
                .into(),
        )
    }

    #[tokio::test]
    async fn price_estimate_uses_best_pool() {
        let token_a = H160([0x0a; 20]);
        let token_b = H160([0x0b; 20]);

        let pools = vec![
            Pool::uniswap(
                TokenPair::new(token_a, token_b).unwrap(),
                (100_000, 100_000),
            ),
            Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (100_000, 90_000)),
        ];

        let pool_fetcher = Arc::new(FakePoolFetcher(pools.clone()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            HashSet::new(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_a,
            10.into(),
        );

        let price = estimator
            .estimate_price(token_a, token_b, 100.into(), OrderKind::Sell)
            .await
            .unwrap();
        // Pool 0 is more favourable for buying token B.
        assert_eq!(price, pool_price(&pools[0], token_b, 100, token_a));

        let price = estimator
            .estimate_price(token_b, token_a, 100.into(), OrderKind::Sell)
            .await
            .unwrap();
        // Pool 1 is more favourable for buying token A.
        assert_eq!(price, pool_price(&pools[1], token_a, 100, token_b));
    }

    #[tokio::test]
    async fn gas_estimate_returns_cost_of_best_path() {
        let token_a = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let token_b = H160::from_low_u64_be(3);

        // Direct trade is better when selling token_b
        let pools = vec![
            Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (1000, 1000)),
            Pool::uniswap(TokenPair::new(token_a, intermediate).unwrap(), (900, 1000)),
            Pool::uniswap(TokenPair::new(intermediate, token_b).unwrap(), (900, 1000)),
        ];

        let pool_fetcher = Arc::new(FakePoolFetcher(pools));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(intermediate),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            intermediate,
            10.into(),
        );

        // Trade with intermediate hop
        for kind in &[OrderKind::Sell, OrderKind::Buy] {
            assert_eq!(
                estimator
                    .estimate_gas(token_a, token_b, 1.into(), *kind)
                    .await
                    .unwrap(),
                260_000.into()
            );
        }

        // Direct Trade
        for kind in &[OrderKind::Sell, OrderKind::Buy] {
            assert_eq!(
                estimator
                    .estimate_gas(token_b, token_a, 10.into(), *kind)
                    .await
                    .unwrap(),
                200_000.into()
            );
        }
    }

    #[tokio::test]
    async fn unsupported_tokens() {
        let supported_token = H160::from_low_u64_be(1);
        let unsupported_token = H160::from_low_u64_be(2);

        let pool_fetcher = Arc::new(FakePoolFetcher(Vec::new()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
            Default::default(),
            1.into(),
        );

        // Price estimate selling unsupported
        assert!(matches!(estimator.estimate_price(
            unsupported_token,
            supported_token,
            100.into(),
            OrderKind::Sell
        ).await, Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token));

        // Price estimate buying unsupported
        assert!(matches!(estimator.estimate_price(
            supported_token,
            unsupported_token,
            100.into(),
            OrderKind::Sell
        ).await, Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token));

        // Gas estimate selling unsupported
        assert!(matches!(estimator.estimate_gas(
            unsupported_token,
            supported_token,
            100.into(),
            OrderKind::Sell
        ).await, Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token));

        // Gas estimate buying unsupported
        assert!(matches!(estimator.estimate_gas(
            supported_token,
            unsupported_token,
            100.into(),
            OrderKind::Sell
        ).await, Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token));
    }

    #[tokio::test]
    async fn price_estimate_takes_gas_costs_into_account() {
        let native = H160::from_low_u64_be(0);
        let sell = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy = H160::from_low_u64_be(3);

        let pools = vec![
            // Native token connection for tokens 1, 2. Note that the connection has a price much
            // worse than the pools between 1, 2, 3 so that it is not used for the trade, just for
            // gas price.
            Pool::uniswap(
                TokenPair::new(native, sell).unwrap(),
                (100_000_000_000, 2_000),
            ),
            Pool::uniswap(
                TokenPair::new(native, buy).unwrap(),
                (100_000_000_000, 1_000),
            ),
            // Direct connection 1 to 3.
            Pool::uniswap(TokenPair::new(sell, buy).unwrap(), (1000, 800)),
            // Intermediate from 1 to 2 to 2, cheaper than direct.
            Pool::uniswap(TokenPair::new(sell, intermediate).unwrap(), (1000, 1000)),
            Pool::uniswap(TokenPair::new(intermediate, buy).unwrap(), (1000, 1000)),
        ];

        let pool_fetcher = Arc::new(FakePoolFetcher(pools.clone()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(10000.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator.clone(),
            hashset!(native, intermediate),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            native,
            1_000_000_000.into(),
        );

        // Uses 1 hop because high gas price doesn't make the intermediate hop worth it.
        for order_kind in [OrderKind::Sell, OrderKind::Buy].iter() {
            assert_eq!(
                estimator
                    .estimate_gas(sell, buy, 10.into(), *order_kind)
                    .await
                    .unwrap(),
                200_000.into()
            );
        }

        // Reduce gas price.
        *gas_estimator.0.lock().unwrap() = 1.0;

        // Lower gas price does make the intermediate hop worth it.
        for order_kind in [OrderKind::Sell, OrderKind::Buy].iter() {
            assert_eq!(
                estimator
                    .estimate_gas(sell, buy, 10.into(), *order_kind)
                    .await
                    .unwrap(),
                260_000.into()
            );
        }
    }

    #[tokio::test]
    async fn estimate_price_honours_parameter_consider_gas_costs() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let token_c = H160::from_low_u64_be(3);

        // A->B->C prices buy token to 1.006 but costs 2*G.
        // A->C prices buy token to 1.007 but costs G.

        let pool_ab = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(28), 10u128.pow(28)),
        );
        let pool_bc = Pool::uniswap(
            TokenPair::new(token_b, token_c).unwrap(),
            (10u128.pow(28), 10u128.pow(28)),
        );
        let pool_ac = Pool::uniswap(
            TokenPair::new(token_a, token_c).unwrap(),
            (1004 * 10u128.pow(25), 10u128.pow(28)),
        );

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool_ab, pool_bc, pool_ac]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(
            1000000000000000.0,
        ))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(token_b),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_a,
            10u128.pow(18).into(),
        );

        let price_considering_gas_costs = estimator
            .estimate_price_helper(
                token_a,
                token_c,
                10u128.pow(19).into(),
                OrderKind::Sell,
                true,
            )
            .await
            .unwrap()
            .1;

        let price_disregarding_gas_costs = estimator
            .estimate_price_helper(
                token_a,
                token_c,
                10u128.pow(19).into(),
                OrderKind::Sell,
                false,
            )
            .await
            .unwrap()
            .1;

        assert!(price_considering_gas_costs != price_disregarding_gas_costs);

        assert!(price_considering_gas_costs <= BigRational::new(1008.into(), 1000.into()));
        assert!(price_disregarding_gas_costs <= BigRational::new(1008.into(), 1000.into()));
    }

    #[tokio::test]
    async fn estimate_price_does_not_panic_on_zero_amount() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_ab = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(18), 1),
        );
        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool_ab]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Default::default()));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(token_b),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_a,
            1.into(),
        );

        let result = estimator
            .estimate_price_helper(
                token_a,
                token_b,
                10u128.pow(18).into(),
                OrderKind::Sell,
                false,
            )
            .await;
        assert!(result.is_err());
    }
}
