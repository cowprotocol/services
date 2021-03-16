use crate::conversions::U256Ext;
use crate::uniswap_pool::{Pool, PoolFetching};
use crate::uniswap_solver::{
    estimate_buy_amount, estimate_sell_amount, estimate_spot_price, path_candidates,
    token_path_to_pair_path,
};
use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use futures::future::join_all;
use model::{order::OrderKind, TokenPair};
use num::{BigRational, ToPrimitive};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};
const MAX_HOPS: usize = 2;

#[async_trait::async_trait]
pub trait PriceEstimating: Send + Sync {
    // Price is given in how much of sell_token needs to be sold for one buy_token.
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational>;

    // Returns the expected gas cost for this given trade
    async fn estimate_gas(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<U256>;

    async fn estimate_price_as_f64(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<f64> {
        self.estimate_price(sell_token, buy_token, amount, kind)
            .await
            .and_then(|price| {
                price
                    .to_f64()
                    .ok_or_else(|| anyhow!("Cannot convert price ratio to float"))
            })
    }

    // Returns a vector of (rational) prices for the given tokens denominated
    // in denominator_token or an error in case there is an error computing any
    // of the prices in the vector.
    async fn estimate_prices(
        &self,
        tokens: &[H160],
        denominator_token: H160,
    ) -> Result<Vec<BigRational>> {
        join_all(tokens.iter().map(|token| async move {
            if *token != denominator_token {
                self.estimate_price(*token, denominator_token, U256::zero(), OrderKind::Buy)
                    .await
            } else {
                Ok(num::one())
            }
        }))
        .await
        .into_iter()
        .collect()
    }
}

pub struct UniswapPriceEstimator {
    pool_fetcher: Box<dyn PoolFetching>,
    base_tokens: HashSet<H160>,
}

impl UniswapPriceEstimator {
    pub fn new(pool_fetcher: Box<dyn PoolFetching>, base_tokens: HashSet<H160>) -> Self {
        Self {
            pool_fetcher,
            base_tokens,
        }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for UniswapPriceEstimator {
    // Estimates the price between sell and buy token denominated in |sell token| per buy token.
    // Returns an error if no path exists between sell and buy token.
    // Incorporates uniswap fee unless amount is 0 in which case it returns the best spot price.
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<BigRational> {
        if sell_token == buy_token {
            return Ok(num::one());
        }
        if amount.is_zero() {
            return self
                .best_execution_spot_price(sell_token, buy_token)
                .await
                .map(|(_, price)| price);
        }
        match kind {
            OrderKind::Buy => {
                let (_, sell_amount) = self
                    .best_execution_buy_order(sell_token, buy_token, amount)
                    .await?;
                Ok(BigRational::new(
                    sell_amount.to_big_int(),
                    amount.to_big_int(),
                ))
            }
            OrderKind::Sell => {
                let (_, buy_amount) = self
                    .best_execution_sell_order(sell_token, buy_token, amount)
                    .await?;
                if buy_amount.is_zero() {
                    return Err(anyhow!(
                        "Attempt to create a rational with zero denominator."
                    ));
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
    ) -> Result<U256> {
        if sell_token == buy_token || amount.is_zero() {
            return Ok(U256::zero());
        }

        let path = match kind {
            OrderKind::Buy => {
                self.best_execution_buy_order(sell_token, buy_token, amount)
                    .await?
                    .0
            }
            OrderKind::Sell => {
                self.best_execution_sell_order(sell_token, buy_token, amount)
                    .await?
                    .0
            }
        };
        let trades = path.len() - 1;
        // This could be more accurate by actually simulating the settlement (since different tokens might have more or less expensive transfer costs)
        // For the standard OZ token the cost is roughly 110k for a direct trade, 170k for a 1 hop trade, 230k for a 2 hop trade.
        return Ok(U256::from(50_000) + 60_000 * trades);
    }
}

impl UniswapPriceEstimator {
    pub async fn best_execution_sell_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        sell_amount: U256,
    ) -> Result<(Vec<H160>, U256)> {
        self.best_execution(
            sell_token,
            buy_token,
            sell_amount,
            estimate_buy_amount,
            estimate_buy_amount,
        )
        .await
    }

    pub async fn best_execution_buy_order(
        &self,
        sell_token: H160,
        buy_token: H160,
        buy_amount: U256,
    ) -> Result<(Vec<H160>, U256)> {
        self.best_execution(
            sell_token,
            buy_token,
            buy_amount,
            |amount, path, pools| {
                Reverse(estimate_sell_amount(amount, path, pools).unwrap_or_else(U256::max_value))
            },
            estimate_sell_amount,
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
            |_, path, pools| estimate_spot_price(path, pools),
            |_, path, pools| estimate_spot_price(path, pools),
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
        AmountFn: Fn(U256, &[H160], &HashMap<TokenPair, Pool>) -> Option<Amount>,
        CompareFn: Fn(U256, &[H160], &HashMap<TokenPair, Pool>) -> O,
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
            .map(|pool| (pool.tokens, pool))
            .collect();
        let best_path = path_candidates
            .iter()
            .max_by_key(|path| comparison(amount, path, &pools))
            .ok_or(anyhow!(format!(
                "No Uniswap path found between {:x} and {:x}",
                sell_token, buy_token
            )))?;
        Ok((
            best_path.clone(),
            resulting_amount(amount, best_path, &pools)
                .ok_or_else(|| anyhow!("no valid path found"))?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;
    use maplit::hashset;
    use std::collections::HashSet;

    use super::*;
    use crate::uniswap_pool::{Pool, PoolFetching};

    struct FakePoolFetcher(Vec<Pool>);
    #[async_trait::async_trait]
    impl PoolFetching for FakePoolFetcher {
        async fn fetch(&self, _: HashSet<TokenPair>) -> Vec<Pool> {
            self.0.clone()
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
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!());

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
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!());

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
        let estimator =
            UniswapPriceEstimator::new(pool_fetcher, hashset!(token_a, token_b, token_c));

        let res = estimator
            .estimate_prices(&[token_a, token_b, token_c], token_c)
            .await;
        assert!(res.is_ok());
        let prices = res.unwrap();
        assert!(prices[0] == BigRational::new(1.into(), 100.into()));
        assert!(prices[1] == BigRational::new(1.into(), 10.into()));
        assert!(prices[2] == BigRational::new(1.into(), 1.into()));
    }

    #[tokio::test]
    async fn return_error_if_no_token_found() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_fetcher = Box::new(FakePoolFetcher(vec![]));
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!());

        assert!(estimator
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Buy)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn return_error_if_invalid_reserves() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (0, 10));

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!());

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
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!(base_token));

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
        let estimator = UniswapPriceEstimator::new(pool_fetcher, hashset!(intermediate));

        // Trade with intermediate hop
        for kind in &[OrderKind::Sell, OrderKind::Buy] {
            assert_eq!(
                estimator
                    .estimate_gas(token_a, token_b, 1.into(), *kind)
                    .await
                    .unwrap(),
                170_000.into()
            );
        }

        // Direct Trade
        for kind in &[OrderKind::Sell, OrderKind::Buy] {
            assert_eq!(
                estimator
                    .estimate_gas(token_b, token_a, 1.into(), *kind)
                    .await
                    .unwrap(),
                110_000.into()
            );
        }
    }
}
