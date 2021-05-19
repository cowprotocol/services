use crate::{
    bad_token::BadTokenDetecting,
    baseline_solver::{
        estimate_buy_amount, estimate_sell_amount, estimate_spot_price, path_candidates,
        token_path_to_pair_path,
    },
    conversions::U256Ext,
    pool_fetching::{Pool, PoolFetching},
};
use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use model::{order::OrderKind, TokenPair};
use num::{BigRational, ToPrimitive};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    sync::Arc,
};
use thiserror::Error;

const MAX_HOPS: usize = 2;

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
    // Price is given in how much of sell_token needs to be sold for one buy_token.
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational, PriceEstimationError>;

    // Returns the expected gas cost for this given trade
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
                self.estimate_price(*token, denominator_token, U256::zero(), OrderKind::Buy)
                    .await
            } else {
                Ok(num::one())
            }
        }))
        .await
    }
}

pub struct BaselinePriceEstimator {
    pool_fetcher: Box<dyn PoolFetching>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    base_tokens: HashSet<H160>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_token: H160,
}

impl BaselinePriceEstimator {
    pub fn new(
        pool_fetcher: Box<dyn PoolFetching>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        base_tokens: HashSet<H160>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        native_token: H160,
    ) -> Self {
        Self {
            pool_fetcher,
            gas_estimator,
            base_tokens,
            bad_token_detector,
            native_token,
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
    // Estimates the price between sell and buy token denominated in |sell token| per buy token.
    // Returns an error if no path exists between sell and buy token.
    // Incorporates uniswap fee unless amount is 0 in which case it returns the best spot price.
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational, PriceEstimationError> {
        self.ensure_token_supported(sell_token).await?;
        self.ensure_token_supported(buy_token).await?;
        if sell_token == buy_token {
            return Ok(num::one());
        }
        if amount.is_zero() {
            return Ok(self
                .best_execution_spot_price(sell_token, buy_token)
                .await
                .map(|(_, price)| price)?);
        }
        let gas_price = self.gas_estimator.estimate().await?;
        match kind {
            OrderKind::Buy => {
                let (_, sell_amount) = self
                    .best_execution_buy_order(sell_token, buy_token, amount, gas_price)
                    .await?;
                Ok(BigRational::new(
                    sell_amount.to_big_int(),
                    amount.to_big_int(),
                ))
            }
            OrderKind::Sell => {
                let (_, buy_amount) = self
                    .best_execution_sell_order(sell_token, buy_token, amount, gas_price)
                    .await?;
                if buy_amount.is_zero() {
                    return Err(
                        anyhow!("Attempt to create a rational with zero denominator.").into(),
                    );
                }
                Ok(BigRational::new(
                    amount.to_big_int(),
                    buy_amount.to_big_int(),
                ))
            }
        }
    }

    async fn estimate_gas(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<U256, PriceEstimationError> {
        self.ensure_token_supported(sell_token).await?;
        self.ensure_token_supported(buy_token).await?;
        if sell_token == buy_token || amount.is_zero() {
            return Ok(U256::zero());
        }

        let gas_price = self.gas_estimator.estimate().await?;
        let path = match kind {
            OrderKind::Buy => {
                self.best_execution_buy_order(sell_token, buy_token, amount, gas_price)
                    .await?
                    .0
            }
            OrderKind::Sell => {
                self.best_execution_sell_order(sell_token, buy_token, amount, gas_price)
                    .await?
                    .0
            }
        };
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
}

impl BaselinePriceEstimator {
    pub async fn best_execution_sell_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        sell_amount: U256,
        gas_price: f64,
    ) -> Result<(Vec<H160>, U256)> {
        // Estimate with amount 0 to get a spot price (avoid potential endless recursion)
        let buy_token_price_in_native_token = self
            .estimate_price(self.native_token, buy_token, U256::zero(), OrderKind::Sell)
            .await?;
        self.best_execution(
            sell_token,
            buy_token,
            sell_amount,
            |amount, path, pools| {
                estimate_buy_amount(amount, path, pools).map(|estimate| {
                    let proceeds_in_native_token =
                        estimate.value.to_big_rational() * buy_token_price_in_native_token.clone();
                    let tx_cost_in_native_token = U256::from_f64_lossy(gas_price).to_big_rational()
                        * BigRational::from_integer(estimate.gas_cost().into());
                    proceeds_in_native_token - tx_cost_in_native_token
                })
            },
            |amount, path, pools| {
                estimate_buy_amount(amount, path, pools).map(|estimate| estimate.value)
            },
        )
        .await
    }

    pub async fn best_execution_buy_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        buy_amount: U256,
        gas_price: f64,
    ) -> Result<(Vec<H160>, U256)> {
        // Estimate with amount 0 to get a spot price (avoid potential endless recursion)
        let sell_token_price_in_eth = self
            .estimate_price(self.native_token, sell_token, U256::zero(), OrderKind::Sell)
            .await?;
        self.best_execution(
            sell_token,
            buy_token,
            buy_amount,
            |amount, path, pools| {
                Reverse(
                    estimate_sell_amount(amount, path, pools)
                        .map(|estimate| {
                            let cost_in_native_token =
                                estimate.value.to_big_rational() * sell_token_price_in_eth.clone();
                            let tx_cost_in_native_token = U256::from_f64_lossy(gas_price)
                                .to_big_rational()
                                * BigRational::from_integer(estimate.gas_cost().into());
                            cost_in_native_token + tx_cost_in_native_token
                        })
                        .unwrap_or_else(|| U256::max_value().to_big_rational()),
                )
            },
            |amount, path, pools| {
                estimate_sell_amount(amount, path, pools).map(|estimate| estimate.value)
            },
        )
        .await
    }

    pub async fn best_execution_spot_price(
        &self,
        sell_token: H160,
        buy_token: H160,
    ) -> Result<(Vec<H160>, BigRational)> {
        self.best_execution(
            sell_token,
            buy_token,
            U256::zero(),
            |_, path, pools| estimate_spot_price(path, pools).map(|estimate| estimate.value),
            |_, path, pools| estimate_spot_price(path, pools).map(|estimate| estimate.value),
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
        let path_candidates = path_candidates(sell_token, buy_token, &self.base_tokens, MAX_HOPS);
        let all_pairs = path_candidates
            .iter()
            .flat_map(|candidate| token_path_to_pair_path(candidate).into_iter())
            .collect();
        let pools: HashMap<_, _> = self
            .pool_fetcher
            .fetch(all_pairs)
            .await
            .into_iter()
            .map(|pool| (pool.tokens, vec![pool]))
            .collect();
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
    }
}

#[cfg(test)]
mod tests {
    use crate::bad_token::list_based::ListBasedDetector;
    use assert_approx_eq::assert_approx_eq;
    use maplit::hashset;
    use std::collections::HashSet;
    use std::sync::Mutex;

    use super::*;
    use crate::{
        gas_price_estimation::FakeGasPriceEstimator,
        pool_fetching::{Pool, PoolFetching},
    };

    struct FakePoolFetcher(Vec<Pool>);
    #[async_trait::async_trait]
    impl PoolFetching for FakePoolFetcher {
        async fn fetch(&self, token_pairs: HashSet<TokenPair>) -> Vec<Pool> {
            self.0
                .clone()
                .into_iter()
                .filter(|pool| token_pairs.contains(&pool.tokens))
                .collect()
        }
    }

    #[tokio::test]
    async fn estimate_price_on_direct_pair() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(30), 10u128.pow(29)),
        );

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_b,
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
    async fn estimate_price_with_zero_amount() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(30), 10u128.pow(29)),
        );

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
        );

        assert!(estimator
            .estimate_price(token_a, token_b, 0.into(), OrderKind::Buy)
            .await
            .is_ok());
        assert!(estimator
            .estimate_price(token_a, token_b, 0.into(), OrderKind::Sell)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn estimate_prices_with_zero_amount() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let token_c = H160::from_low_u64_be(3);
        let pool_ab = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (10u128.pow(30), 10u128.pow(29)),
        );
        let pool_bc = Pool::uniswap(
            TokenPair::new(token_b, token_c).unwrap(),
            (10u128.pow(30), 10u128.pow(29)),
        );

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool_ab, pool_bc]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(token_a, token_b, token_c),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
        );

        let prices = estimator
            .estimate_prices(&[token_a, token_b, token_c], token_c)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(prices[0], BigRational::new(1.into(), 100.into()));
        assert_eq!(prices[1], BigRational::new(1.into(), 10.into()));
        assert_eq!(prices[2], BigRational::new(1.into(), 1.into()));
    }

    #[tokio::test]
    async fn return_error_if_no_token_found() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_fetcher = Box::new(FakePoolFetcher(vec![]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
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
            (10u128.pow(30), 10u128.pow(29)),
        );
        let pool_fetcher = FakePoolFetcher(vec![pool_ab]);
        let bad_token = Arc::new(ListBasedDetector::deny_list(vec![token_a]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            Box::new(pool_fetcher),
            gas_estimator,
            hashset!(),
            bad_token,
            token_a,
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

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
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
            (10u128.pow(30), 10u128.pow(29)),
        );

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(base_token),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            token_b,
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

        let pool_fetcher = Box::new(FakePoolFetcher(pools));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(intermediate),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            intermediate,
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
                    .estimate_gas(token_b, token_a, 1.into(), *kind)
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

        let pool_fetcher = Box::new(FakePoolFetcher(Vec::new()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
            Default::default(),
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
        let token_a = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let token_b = H160::from_low_u64_be(3);

        // Multi-hop offers better price when selling a for b
        let pools = vec![
            Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (100_000, 90_000)),
            Pool::uniswap(
                TokenPair::new(token_a, intermediate).unwrap(),
                (100_000, 100_000),
            ),
            Pool::uniswap(
                TokenPair::new(intermediate, token_b).unwrap(),
                (100_000, 100_000),
            ),
        ];

        let pool_fetcher = Box::new(FakePoolFetcher(pools.clone()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(100.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(intermediate),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            intermediate,
        );

        // Only use 1 hop
        assert_eq!(
            estimator
                .estimate_gas(token_a, token_b, 100.into(), OrderKind::Sell)
                .await
                .unwrap(),
            200_000.into()
        );

        // Only use 1 hop
        assert_eq!(
            estimator
                .estimate_gas(token_a, token_b, 100.into(), OrderKind::Buy)
                .await
                .unwrap(),
            200_000.into()
        );

        // Now with a cheap gas price
        let pool_fetcher = Box::new(FakePoolFetcher(pools));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(intermediate),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            intermediate,
        );

        // Use 2 hops
        assert_eq!(
            estimator
                .estimate_gas(token_a, token_b, 100.into(), OrderKind::Sell)
                .await
                .unwrap(),
            260_000.into()
        );

        // Use 2 hops
        assert_eq!(
            estimator
                .estimate_gas(token_a, token_b, 100.into(), OrderKind::Buy)
                .await
                .unwrap(),
            260_000.into()
        );
    }
}
