use anyhow::{anyhow, ensure, Result};
use ethcontract::H160;
use model::TokenPair;
use shared::uniswap_pool::PoolFetching;
use std::iter::once;

#[allow(dead_code)]
struct UniswapPriceEstimator {
    pool_fetcher: Box<dyn PoolFetching>,
}

impl UniswapPriceEstimator {
    // Estimates the price using the direct pool between sell and buy token. Price is given in
    // how much of sell_token needs to be sold for one buy_token.
    // Returns an error if no pool exists between sell and buy token.
    #[allow(dead_code)]
    pub async fn estimate_price(&self, sell_token: H160, buy_token: H160) -> Result<f64> {
        let pair = match TokenPair::new(sell_token, buy_token) {
            Some(pair) => pair,
            None => return Ok(1.0),
        };
        let pool = self
            .pool_fetcher
            .fetch(once(pair).collect())
            .await
            .pop()
            .ok_or_else(|| anyhow!("Uniswap pool does not exist"))?;
        let (sell_reserve, buy_reserve) = if pool.tokens.get().0 == sell_token {
            (pool.reserves.0, pool.reserves.1)
        } else {
            (pool.reserves.1, pool.reserves.0)
        };
        ensure!(
            sell_reserve != 0 && buy_reserve != 0,
            "Pools with empty reserve"
        );
        Ok((sell_reserve as f64) / (buy_reserve as f64))
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;
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
        let pool = Pool {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (100, 10),
        };

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let estimator = UniswapPriceEstimator { pool_fetcher };

        assert_approx_eq!(
            estimator.estimate_price(token_a, token_a).await.unwrap(),
            1.0
        );
        assert_approx_eq!(
            estimator.estimate_price(token_a, token_b).await.unwrap(),
            10.0
        );
        assert_approx_eq!(
            estimator.estimate_price(token_b, token_a).await.unwrap(),
            0.1
        );
    }

    #[tokio::test]
    async fn return_error_if_no_token_found() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool_fetcher = Box::new(FakePoolFetcher(vec![]));
        let estimator = UniswapPriceEstimator { pool_fetcher };

        assert!(estimator.estimate_price(token_a, token_b).await.is_err());
    }

    #[tokio::test]
    async fn return_error_if_invalid_reserves() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pool = Pool {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (0, 10),
        };

        let pool_fetcher = Box::new(FakePoolFetcher(vec![pool]));
        let estimator = UniswapPriceEstimator { pool_fetcher };

        assert!(estimator.estimate_price(token_a, token_b).await.is_err());
    }
}
