use crate::{
    bad_token::BadTokenDetecting,
    baseline_solver::{
        estimate_buy_amount, estimate_sell_amount, estimate_spot_price, path_candidates,
        token_path_to_pair_path, DEFAULT_MAX_HOPS,
    },
    conversions::U256Ext,
    recent_block_cache::Block,
    sources::uniswap::pool_fetching::{Pool, PoolFetching},
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
    /// If amount is 0 then kind is ignored and the result is how many units of buy token you get
    /// from selling one atom of sell token (buy_amount / sell_amount).
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

    // Default is to estimate prices with 1 atom of native token (i.e. spot prices)
    fn amount_to_estimate_prices_with(
        &self
    ) -> U256 {
        1.into()
    }

    // Returns a vector of (rational) prices for the given tokens denominated
    // in denominator_token or an error in case there is an error computing any
    // of the prices in the vector.
    async fn estimate_prices_0(
        &self,
        tokens: &[H160],
        denominator_token: H160,
    ) -> Vec<Result<BigRational, PriceEstimationError>> {
        join_all(tokens.iter().map(|token| async move {
            if *token != denominator_token {
                self.estimate_price(*token, denominator_token, 0.into(), OrderKind::Sell)
                    .await
            } else {
                Ok(num::one())
            }
        }))
        .await
    }

    // Returns a vector of (rational) prices for the given tokens denominated
    // in denominator_token or an error in case there is an error computing any
    // of the prices in the vector.
    async fn estimate_prices_1(
        &self,
        tokens: &[H160],
        denominator_token: H160,
        amount:U256,
    ) -> Vec<Result<BigRational, PriceEstimationError>> {
        // let amount = self.amount_to_estimate_prices_with();
        join_all(tokens.iter().map(|token| async move {
            if *token != denominator_token {
                self.estimate_price(denominator_token, *token, amount, OrderKind::Sell)
                    .await
            } else {
                Ok(num::one())
            }
        }))
        .await
    }

    async fn estimate_prices(
        &self,
        tokens: &[H160],
        denominator_token: H160,
    ) -> Vec<Result<BigRational, PriceEstimationError>> {
        let prices0 = self.estimate_prices_0(tokens, denominator_token).await;
        let prices1 = self.estimate_prices_1(tokens, denominator_token, 1.into()).await;
        let prices2 = self.estimate_prices_1(tokens, denominator_token, "1000000000000000000".into()).await;
        println!("-----------");
        for (t, p0, p1, p2) in itertools::izip!(tokens, prices0, prices1, prices2) {
            println!("{:#?} : {:#?} {:#?} {:#?}", t, p0, p1, p2);
        }
        self.estimate_prices_1(tokens, denominator_token, "1000000000000000000".into()).await
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
        amount_to_estimate_prices_with : U256,
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

    fn amount_to_estimate_prices_with(
        &self
    ) -> U256 {
        self.amount_to_estimate_prices_with
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
            .estimate_price(buy_token, self.native_token, U256::zero(), OrderKind::Sell)
            .await?;

        self.best_execution(
            sell_token,
            buy_token,
            sell_amount,
            |amount, path, pools| {
                estimate_buy_amount(amount, path, pools).map(|estimate| {
                    /*
                    let proceeds_in_native_token =
                        estimate.value.to_big_rational() * buy_token_price_in_native_token.clone();
                    let tx_cost_in_native_token = U256::from_f64_lossy(gas_price).to_big_rational()
                        * BigRational::from_integer(estimate.gas_cost().into());
                    proceeds_in_native_token - tx_cost_in_native_token*/
                    estimate.value.to_big_rational()
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
            .estimate_price(sell_token, self.native_token, U256::zero(), OrderKind::Sell)
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
            0.into(),
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
            (10u128.pow(28), 10u128.pow(27)),
        );

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
            0.into(),
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
            (10u128.pow(28), 10u128.pow(27)),
        );
        let pool_bc = Pool::uniswap(
            TokenPair::new(token_b, token_c).unwrap(),
            (10u128.pow(28), 10u128.pow(27)),
        );

        let pool_fetcher = Arc::new(FakePoolFetcher(vec![pool_ab, pool_bc]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(token_a, token_b, token_c),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
            0.into(),
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
        let pool_fetcher = Arc::new(FakePoolFetcher(vec![]));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(Vec::new())),
            Default::default(),
            0.into(),
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
            0.into(),
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
            Default::default(),
            0.into(),
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
            0.into(),
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
            0.into(),
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
            0.into(),
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

        let pool_fetcher = Arc::new(FakePoolFetcher(Vec::new()));
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(0.0))));
        let estimator = BaselinePriceEstimator::new(
            pool_fetcher,
            gas_estimator,
            hashset!(),
            Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
            Default::default(),
            0.into(),
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
            0.into(),
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
}
