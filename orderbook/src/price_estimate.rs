use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use model::{order::OrderKind, TokenPair};
use shared::{
    uniswap_pool::{Pool, PoolFetching},
    uniswap_solver::{
        estimate_buy_amount, estimate_sell_amount, path_candidates, token_path_to_pair_path,
    },
};
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
    ) -> Result<f64>;
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
    async fn estimate_price(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<f64> {
        if sell_token == buy_token {
            return Ok(1.0);
        }
        let amount = U256::max(amount, U256::one());

        match kind {
            OrderKind::Buy => {
                let (_, sell_amount) = self
                    .best_execution_buy_order(sell_token, buy_token, amount)
                    .await?;
                Ok(sell_amount.to_f64_lossy() / amount.to_f64_lossy())
            }
            OrderKind::Sell => {
                let (_, buy_amount) = self
                    .best_execution_sell_order(sell_token, buy_token, amount)
                    .await?;
                Ok(amount.to_f64_lossy() / buy_amount.to_f64_lossy())
            }
        }
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

    async fn best_execution<AmountFn, CompareFn, O>(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        comparison: CompareFn,
        resulting_amount: AmountFn,
    ) -> Result<(Vec<H160>, U256)>
    where
        AmountFn: Fn(U256, &[H160], &HashMap<TokenPair, Pool>) -> Option<U256>,
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
    use shared::uniswap_pool::{Pool, PoolFetching};

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
                .estimate_price(token_a, token_a, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            1.0
        );
        assert_approx_eq!(
            estimator
                .estimate_price(token_a, token_a, U256::exp10(18), OrderKind::Sell)
                .await
                .unwrap(),
            1.0
        );
        assert_approx_eq!(
            estimator
                .estimate_price(token_a, token_b, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            10.03,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price(token_a, token_b, U256::exp10(18), OrderKind::Sell)
                .await
                .unwrap(),
            10.03,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price(token_b, token_a, U256::exp10(18), OrderKind::Buy)
                .await
                .unwrap(),
            0.1003,
            1.0e-4
        );
        assert_approx_eq!(
            estimator
                .estimate_price(token_b, token_a, U256::exp10(18), OrderKind::Sell)
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
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Sell)
            .await
            .is_ok());
        assert!(estimator
            .estimate_price(token_a, token_b, 1.into(), OrderKind::Buy)
            .await
            .is_ok());
    }
}
