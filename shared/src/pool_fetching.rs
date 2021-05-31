use crate::{
    amm_pair_provider::AmmPairProvider,
    baseline_solver::BaselineSolvable,
    current_block::{Block as CurrentBlock, CurrentBlockStream},
    ethcontract_error::EthcontractErrorType,
    Web3,
};
use anyhow::Result;
use contracts::{IUniswapLikePair, ERC20};
use ethcontract::{batch::CallBatch, errors::MethodError, BlockNumber, H160, U256};
use lru::LruCache;
use model::TokenPair;
use num::{rational::Ratio, BigInt, BigRational, Zero};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

const MAX_BATCH_SIZE: usize = 100;
const POOL_SWAP_GAS_COST: usize = 60_000;

/// This type denotes `(reserve_a, reserve_b, token_b)` where
/// `reserve_a` refers to the reserve of the excluded token.
type RelativeReserves = (U256, U256, H160);

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: BlockNumber,
    ) -> Result<Vec<Pool>>;
}

#[derive(Clone, Hash, PartialEq, Debug)]
pub struct Pool {
    pub tokens: TokenPair,
    pub reserves: (u128, u128),
    pub fee: Ratio<u32>,
}

impl Pool {
    pub fn uniswap(tokens: TokenPair, reserves: (u128, u128)) -> Self {
        Self {
            tokens,
            reserves,
            fee: Ratio::new(3, 1000),
        }
    }

    /// Given an input amount and token, returns the maximum output amount and address of the other asset.
    /// Returns None if operation not possible due to arithmetic issues (e.g. over or underflow)
    fn get_amount_out(&self, token_in: H160, amount_in: U256) -> Option<(U256, H160)> {
        let (reserve_in, reserve_out, token_out) = self.get_relative_reserves(token_in);
        Some((
            self.amount_out(amount_in, reserve_in, reserve_out)?,
            token_out,
        ))
    }

    /// Given an output amount and token, returns a required input amount and address of the other asset.
    /// Returns None if operation not possible due to arithmetic issues (e.g. over or underflow, reserve too small)
    fn get_amount_in(&self, token_out: H160, amount_out: U256) -> Option<(U256, H160)> {
        let (reserve_out, reserve_in, token_in) = self.get_relative_reserves(token_out);
        Some((
            self.amount_in(amount_out, reserve_in, reserve_out)?,
            token_in,
        ))
    }

    /// Given one of the pool's two tokens, returns a tuple containing the `RelativeReserves`
    /// along with the opposite token. That is, the elements returned are (respectively)
    /// - the pool's reserve of token provided
    /// - the reserve of the other token
    /// - the pool's other token
    /// This is essentially a helper method for shuffling values in `get_amount_in` and `get_amount_out`
    fn get_relative_reserves(&self, token: H160) -> RelativeReserves {
        // https://github.com/Uniswap/uniswap-v2-periphery/blob/master/contracts/libraries/UniswapV2Library.sol#L53
        if token == self.tokens.get().0 {
            (
                U256::from(self.reserves.0),
                U256::from(self.reserves.1),
                self.tokens.get().1,
            )
        } else {
            assert_eq!(token, self.tokens.get().1, "Token not part of pool");
            (
                U256::from(self.reserves.1),
                U256::from(self.reserves.0),
                self.tokens.get().0,
            )
        }
    }

    // Given the base token returns the price (as defined in https://www.investopedia.com/terms/c/currencypair.asp#mntl-sc-block_1-0-18)
    // and quote token. E.g. for the EUR/USD pool with balances 100 (base, EUR) & 125 (quote, USD) the spot price is 125/100
    fn get_spot_price(&self, base_token: H160) -> Option<(BigRational, H160)> {
        let (reserve_base, reserve_quote, quote_token) = if base_token == self.tokens.get().0 {
            (
                BigInt::from(self.reserves.0),
                BigInt::from(self.reserves.1),
                self.tokens.get().1,
            )
        } else {
            assert_eq!(base_token, self.tokens.get().1, "Token not part of pool");
            (
                BigInt::from(self.reserves.1),
                BigInt::from(self.reserves.0),
                self.tokens.get().0,
            )
        };
        if reserve_base == BigInt::zero() {
            return None;
        }

        Some((BigRational::new(reserve_quote, reserve_base), quote_token))
    }

    fn amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> Option<U256> {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return None;
        }

        let amount_in_with_fee =
            amount_in.checked_mul(U256::from(self.fee.denom().checked_sub(*self.fee.numer())?))?;
        let numerator = amount_in_with_fee.checked_mul(reserve_out)?;
        let denominator = reserve_in
            .checked_mul(U256::from(*self.fee.denom()))?
            .checked_add(amount_in_with_fee)?;
        numerator.checked_div(denominator)
    }

    fn amount_in(&self, amount_out: U256, reserve_in: U256, reserve_out: U256) -> Option<U256> {
        if amount_out.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return None;
        }

        let numerator = reserve_in
            .checked_mul(amount_out)?
            .checked_mul(U256::from(*self.fee.denom()))?;
        let denominator = reserve_out
            .checked_sub(amount_out)?
            .checked_mul(U256::from(self.fee.denom().checked_sub(*self.fee.numer())?))?;
        numerator.checked_div(denominator)?.checked_add(1.into())
    }
}

impl BaselineSolvable for Pool {
    fn get_amount_in(&self, in_token: H160, out_amount: U256, out_token: H160) -> Option<U256> {
        self.get_amount_in(out_token, out_amount)
            .map(|(in_amount, token)| {
                assert_eq!(token, in_token);
                in_amount
            })
    }

    fn get_amount_out(&self, out_token: H160, in_amount: U256, in_token: H160) -> Option<U256> {
        self.get_amount_out(in_token, in_amount)
            .map(|(out_amount, token)| {
                assert_eq!(token, out_token);
                out_amount
            })
    }

    fn get_spot_price(&self, base_token: H160, quote_token: H160) -> Option<BigRational> {
        self.get_spot_price(base_token).map(|(price, token)| {
            assert_eq!(token, quote_token);
            price
        })
    }

    fn gas_cost(&self) -> usize {
        POOL_SWAP_GAS_COST
    }
}

const MAX_CACHED_BLOCKS: usize = 10;

// Read though Pool Fetcher that keeps previously fetched pools in a LRU cache. Pools fetched for `BlockNumber::Latest` get invalidated whenever there is a new block
pub struct CachedPoolFetcher {
    inner: Box<dyn PoolFetching>,
    cache: Arc<Mutex<Cache>>,
    block_stream: CurrentBlockStream,
}

struct Cache {
    /// Used to store details (e.g. hash) about the latest block. Needed so we know what `BlockNumber::Latest` refers to
    latest_block: CurrentBlock,
    pools: LruCache<u64, HashMap<TokenPair, Vec<Pool>>>,
}

impl Cache {
    fn latest_block_number(&self) -> u64 {
        self.latest_block
            .number
            .expect("Latest block always has a number")
            .as_u64()
    }
}

impl CachedPoolFetcher {
    pub fn new(inner: Box<dyn PoolFetching>, block_stream: CurrentBlockStream) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(Cache {
                latest_block: CurrentBlock::default(),
                pools: LruCache::new(MAX_CACHED_BLOCKS),
            })),
            block_stream,
        }
    }

    async fn fetch_inner(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: BlockNumber,
    ) -> Result<Vec<Pool>> {
        let mut cache = self.cache.lock().await;
        let block = match at_block {
            BlockNumber::Earliest => 0,
            BlockNumber::Number(number) => number.as_u64(),
            BlockNumber::Latest => cache.latest_block_number(),
            BlockNumber::Pending => {
                tracing::warn!("Pending block not supported by cache");
                return self.inner.fetch(token_pairs, at_block).await;
            }
        };

        let cached_pools = match cache.pools.get_mut(&block) {
            Some(cache) => cache,
            None => {
                cache.pools.put(block, Default::default());
                cache.pools.get_mut(&block).unwrap()
            }
        };

        let mut cache_hits = Vec::new();
        let mut cache_misses = HashSet::new();
        for &pair in &token_pairs {
            match cached_pools.entry(pair) {
                Entry::Occupied(entry) => {
                    cache_hits.extend_from_slice(entry.get());
                }
                Entry::Vacant(entry) => {
                    cache_misses.insert(pair);
                    // It is possible that the inner fetcher when queried below does not return any
                    // pools for a token pair. It is important that this information itself enters
                    // the cache so that it is remembered on next fetch and we do not attempt to
                    // fetch pools for the token pair again (until the cache expires).
                    entry.insert(Default::default());
                }
            }
        }

        if cache_misses.is_empty() {
            return Ok(cache_hits);
        }

        let mut inner_results = self.inner.fetch(cache_misses, at_block).await?;
        for miss in &inner_results {
            // Unwrap because the loop above guarantees that the cache has an entry for all pairs.
            cached_pools
                .get_mut(&miss.tokens)
                .unwrap()
                .push(miss.clone());
        }

        inner_results.append(&mut cache_hits);
        Ok(inner_results)
    }

    async fn clear_cache_if_necessary(&self) {
        let mut cache = self.cache.lock().await;
        let current = self.block_stream.borrow().clone();
        if cache.latest_block != current {
            cache.latest_block = current;
            // Make sure we don't keep any cached data at that block around
            let number = cache.latest_block_number();
            cache.pools.pop(&number);
        }
    }
}

#[async_trait::async_trait]
impl PoolFetching for CachedPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: BlockNumber,
    ) -> Result<Vec<Pool>> {
        self.clear_cache_if_necessary().await;
        self.fetch_inner(token_pairs, at_block).await
    }
}

pub struct PoolFetcher {
    pub pair_provider: Arc<dyn AmmPairProvider>,
    pub web3: Web3,
}

#[async_trait::async_trait]
impl PoolFetching for PoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: BlockNumber,
    ) -> Result<Vec<Pool>> {
        let mut batch = CallBatch::new(self.web3.transport());
        let futures = token_pairs
            .into_iter()
            .map(|pair| {
                let pair_address = self.pair_provider.pair_address(&pair);
                let pair_contract = IUniswapLikePair::at(&self.web3, pair_address);

                // Fetch ERC20 token balances of the pools to sanity check with reserves
                let token0 = ERC20::at(&self.web3, pair.get().0);
                let token1 = ERC20::at(&self.web3, pair.get().1);

                let reserves = pair_contract
                    .get_reserves()
                    .block(at_block.into())
                    .batch_call(&mut batch);
                let token0_balance = token0
                    .balance_of(pair_address)
                    .block(at_block.into())
                    .batch_call(&mut batch);
                let token1_balance = token1
                    .balance_of(pair_address)
                    .block(at_block.into())
                    .batch_call(&mut batch);

                async move {
                    // Clippy is wrong about this being eval order depndendent.
                    #[allow(clippy::eval_order_dependence)]
                    FetchedPool {
                        pair,
                        reserves: reserves.await,
                        token0_balance: token0_balance.await,
                        token1_balance: token1_balance.await,
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let mut results = Vec::new();
        for future in futures {
            results.push(future.await);
        }
        handle_results(results)
    }
}

struct FetchedPool {
    pair: TokenPair,
    reserves: Result<(u128, u128, u32), MethodError>,
    token0_balance: Result<U256, MethodError>,
    token1_balance: Result<U256, MethodError>,
}

fn handle_results(results: Vec<FetchedPool>) -> Result<Vec<Pool>> {
    // Node errors should be bubbled up but contract errors should lead to the pool being skipped.
    fn handle_contract_error<T>(result: Result<T, MethodError>) -> Result<Option<T>> {
        match result {
            Ok(t) => Ok(Some(t)),
            Err(err) => match EthcontractErrorType::classify(&err) {
                EthcontractErrorType::Node => Err(err.into()),
                EthcontractErrorType::Contract => Ok(None),
            },
        }
    }

    results.into_iter().try_fold(Vec::new(), |mut acc, pool| {
        let reserves = match handle_contract_error(pool.reserves)? {
            Some(reserves) => reserves,
            None => return Ok(acc),
        };
        let token0_balance = match handle_contract_error(pool.token0_balance)? {
            Some(balance) => balance,
            None => return Ok(acc),
        };
        let token1_balance = match handle_contract_error(pool.token1_balance)? {
            Some(balance) => balance,
            None => return Ok(acc),
        };
        // Some ERC20s (e.g. AMPL) have an elastic supply and can thus reduce the balance of their owners without any transfer or other interaction ("rebase").
        // Such behavior can implicitly change the *k* in the pool's constant product formula. E.g. a pool with 10 USDC and 10 AMPL has k = 100. After a negative
        // rebase the pool's AMPL balance may reduce to 9, thus k should be implicitly updated to 90 (figuratively speaking the pool is undercollateralized).
        // Uniswap pools however only update their reserves upon swaps. Such an "out of sync" pool has numerical issues when computing the right clearing price.
        // Note, that a positive rebase is not problematic as k would increase in this case giving the pool excess in the elastic token (an arbitrageur could
        // benefit by withdrawing the excess from the pool without selling anything).
        // We therefore exclude all pools where the pool's token balance of either token in the pair is less than the cached reserve.
        if U256::from(reserves.0) <= token0_balance && U256::from(reserves.1) <= token1_balance {
            acc.push(Pool::uniswap(pool.pair, (reserves.0, reserves.1)));
        }
        Ok(acc)
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{conversions::big_rational_to_float, ethcontract_error};
    use assert_approx_eq::assert_approx_eq;
    use ethcontract::H256;
    use maplit::hashset;
    use tokio::sync::watch;

    #[test]
    fn test_get_amounts_out() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        // Even Pool
        let pool = Pool::uniswap(TokenPair::new(sell_token, buy_token).unwrap(), (100, 100));
        assert_eq!(
            pool.get_amount_out(sell_token, 10.into()),
            Some((9.into(), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 100.into()),
            Some((49.into(), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 1000.into()),
            Some((90.into(), buy_token))
        );

        //Uneven Pool
        let pool = Pool::uniswap(TokenPair::new(sell_token, buy_token).unwrap(), (200, 50));
        assert_eq!(
            pool.get_amount_out(sell_token, 10.into()),
            Some((2.into(), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 100.into()),
            Some((16.into(), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 1000.into()),
            Some((41.into(), buy_token))
        );

        // Large Numbers
        let pool = Pool::uniswap(
            TokenPair::new(sell_token, buy_token).unwrap(),
            (u128::max_value(), u128::max_value()),
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 10u128.pow(20).into()),
            Some((99_699_999_999_999_999_970u128.into(), buy_token))
        );

        // Overflow
        assert_eq!(pool.get_amount_out(sell_token, U256::max_value()), None);
    }

    #[test]
    fn test_get_amounts_in() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        // Even Pool
        let pool = Pool::uniswap(TokenPair::new(sell_token, buy_token).unwrap(), (100, 100));
        assert_eq!(
            pool.get_amount_in(buy_token, 10.into()),
            Some((12.into(), sell_token))
        );
        assert_eq!(
            pool.get_amount_in(buy_token, 99.into()),
            Some((9930.into(), sell_token))
        );

        // Buying more than possible
        assert_eq!(pool.get_amount_in(buy_token, 100.into()), None);
        assert_eq!(pool.get_amount_in(buy_token, 1000.into()), None);

        //Uneven Pool
        let pool = Pool::uniswap(TokenPair::new(sell_token, buy_token).unwrap(), (200, 50));
        assert_eq!(
            pool.get_amount_in(buy_token, 10.into()),
            Some((51.into(), sell_token))
        );
        assert_eq!(
            pool.get_amount_in(buy_token, 49.into()),
            Some((9830.into(), sell_token))
        );

        // Large Numbers
        let pool = Pool::uniswap(
            TokenPair::new(sell_token, buy_token).unwrap(),
            (u128::max_value(), u128::max_value()),
        );
        assert_eq!(
            pool.get_amount_in(buy_token, 10u128.pow(20).into()),
            Some((100_300_902_708_124_373_149u128.into(), sell_token))
        );
    }

    #[test]
    fn test_spot_price() {
        // Example from https://www.investopedia.com/terms/c/currencypair.asp#mntl-sc-block_1-0-18
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);

        let pool = Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (100, 125));
        assert_approx_eq!(
            big_rational_to_float(&pool.get_spot_price(token_a).unwrap().0).unwrap(),
            1.25
        );
        assert_approx_eq!(
            big_rational_to_float(&pool.get_spot_price(token_b).unwrap().0).unwrap(),
            0.8
        );

        assert_eq!(pool.get_spot_price(token_a).unwrap().1, token_b);
        assert_eq!(pool.get_spot_price(token_b).unwrap().1, token_a);

        let pool = Pool::uniswap(TokenPair::new(token_a, token_b).unwrap(), (0, 0));
        assert_eq!(pool.get_spot_price(token_a), None);
    }

    struct FakePoolFetcher(Arc<Mutex<Vec<Pool>>>);
    #[async_trait::async_trait]
    impl PoolFetching for FakePoolFetcher {
        async fn fetch(&self, _: HashSet<TokenPair>, _: BlockNumber) -> Result<Vec<Pool>> {
            Ok(self.0.lock().await.clone())
        }
    }

    #[tokio::test]
    async fn caching_pool_fetcher() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let pools = Arc::new(Mutex::new(vec![
            Pool::uniswap(pair, (1, 1)),
            Pool::uniswap(pair, (2, 2)),
        ]));

        let starting_block = CurrentBlock {
            hash: Some(H256::from_low_u64_be(0)),
            number: Some(0.into()),
            ..Default::default()
        };

        let (sender, receiver) = watch::channel(starting_block);
        let inner = Box::new(FakePoolFetcher(pools.clone()));
        let instance = CachedPoolFetcher::new(inner, receiver);

        // Read Through
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Latest)
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1)), Pool::uniswap(pair, (2, 2))]
        );
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Number(42.into()))
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1)), Pool::uniswap(pair, (2, 2))]
        );

        // clear inner to test caching
        pools.lock().await.clear();
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Latest)
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1)), Pool::uniswap(pair, (2, 2))]
        );
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Number(42.into()))
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1)), Pool::uniswap(pair, (2, 2))]
        );

        // invalidate cache
        sender
            .send(CurrentBlock {
                hash: Some(H256::from_low_u64_be(1)),
                number: Some(1.into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Latest)
                .await
                .unwrap(),
            vec![]
        );

        // Cache entry for fixed block didn't change
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Number(42.into()))
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1)), Pool::uniswap(pair, (2, 2))]
        );
    }

    #[tokio::test]
    async fn caching_pool_fetcher_doesnt_cache_pending() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let pools = Arc::new(Mutex::new(vec![Pool::uniswap(pair, (1, 1))]));

        let starting_block = CurrentBlock {
            hash: Some(H256::from_low_u64_be(0)),
            number: Some(0.into()),
            ..Default::default()
        };

        let (_sender, receiver) = watch::channel(starting_block);
        let inner = Box::new(FakePoolFetcher(pools.clone()));
        let instance = CachedPoolFetcher::new(inner, receiver);

        // Read Through
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Pending)
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1))]
        );

        // clear inner to test we are not using cache
        pools.lock().await.clear();
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Pending)
                .await
                .unwrap(),
            vec![]
        );
    }

    #[tokio::test]
    async fn caching_pool_fetcher_invalidates_if_latest_block_reorgs() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let pools = Arc::new(Mutex::new(vec![Pool::uniswap(pair, (1, 1))]));

        let starting_block = CurrentBlock {
            hash: Some(H256::from_low_u64_be(0)),
            number: Some(0.into()),
            ..Default::default()
        };

        let (sender, receiver) = watch::channel(starting_block.clone());
        let inner = Box::new(FakePoolFetcher(pools.clone()));
        let instance = CachedPoolFetcher::new(inner, receiver);

        // Read Through
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Latest)
                .await
                .unwrap(),
            vec![Pool::uniswap(pair, (1, 1))]
        );

        // simulate reorg on latest block
        sender
            .send(CurrentBlock {
                hash: Some(H256::from_low_u64_be(1)),
                number: starting_block.number,
                ..Default::default()
            })
            .unwrap();

        // clear inner, to test we are not using cache
        pools.lock().await.clear();
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Number(0.into()))
                .await
                .unwrap(),
            vec![]
        );
        assert_eq!(
            instance
                .fetch(hashset!(pair), BlockNumber::Latest)
                .await
                .unwrap(),
            vec![]
        );
    }

    #[tokio::test]
    async fn caching_pool_fetcher_caches_empty() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let pools = Arc::new(Mutex::new(vec![]));

        let starting_block = CurrentBlock {
            hash: Some(H256::from_low_u64_be(0)),
            number: Some(0.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(starting_block);
        let inner = Box::new(FakePoolFetcher(pools.clone()));
        let instance = CachedPoolFetcher::new(inner, receiver);

        assert!(instance
            .fetch(hashset!(pair), BlockNumber::Latest)
            .await
            .unwrap()
            .is_empty());
        // Change inner to return a new pool if it was called.
        pools.lock().await.push(Pool::uniswap(pair, (1, 1)));
        // Inner shouldn't get called because the previous empty result is still cached.
        assert!(instance
            .fetch(hashset!(pair), BlockNumber::Latest)
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let results = vec![FetchedPool {
            reserves: Err(ethcontract_error::testing_node_error()),
            pair: Default::default(),
            token0_balance: Ok(1.into()),
            token1_balance: Ok(1.into()),
        }];
        assert!(handle_results(results).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let results = vec![
            FetchedPool {
                reserves: Err(ethcontract_error::testing_contract_error()),
                pair: Default::default(),
                token0_balance: Ok(1.into()),
                token1_balance: Ok(1.into()),
            },
            FetchedPool {
                reserves: Ok((1, 1, 0)),
                pair: Default::default(),
                token0_balance: Ok(1.into()),
                token1_balance: Ok(1.into()),
            },
        ];
        assert_eq!(handle_results(results).unwrap().len(), 1);
    }
}
