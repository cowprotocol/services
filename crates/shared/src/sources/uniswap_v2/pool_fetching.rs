use {
    super::pair_provider::PairProvider,
    crate::{baseline_solver::BaselineSolvable, ethrpc::Web3, recent_block_cache::Block},
    alloy::{
        eips::BlockId,
        primitives::{Address, U256},
    },
    anyhow::Result,
    cached::{Cached, TimedCache},
    contracts::alloy::{
        ERC20,
        IUniswapLikePair::{self, IUniswapLikePair::getReservesReturn},
    },
    ethrpc::alloy::errors::ignore_non_node_error,
    futures::{FutureExt as _, TryStreamExt, future::BoxFuture, stream::FuturesUnordered},
    model::TokenPair,
    num::rational::Ratio,
    std::{
        collections::HashSet,
        sync::{LazyLock, RwLock},
        time::Duration,
    },
    tracing::instrument,
};

const POOL_SWAP_GAS_COST: usize = 60_000;

static POOL_MAX_RESERVES: LazyLock<U256> = LazyLock::new(|| U256::from((1u128 << 112) - 1));

/// This type denotes `(reserve_a, reserve_b, token_b)` where
/// `reserve_a` refers to the reserve of the excluded token.
type RelativeReserves = (U256, U256, Address);

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>>;
}

/// Trait for abstracting the on-chain reading logic for pool state.
pub trait PoolReading: Send + Sync {
    /// Read the pool state for the specified token pair.
    fn read_state(&self, pair: TokenPair, block: BlockId) -> BoxFuture<'_, Result<Option<Pool>>>;
}

impl PoolReading for Box<dyn PoolReading> {
    fn read_state(&self, pair: TokenPair, block: BlockId) -> BoxFuture<'_, Result<Option<Pool>>> {
        (**self).read_state(pair, block)
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub struct Pool {
    pub address: Address,
    pub tokens: TokenPair,
    pub reserves: (u128, u128),
    pub fee: Ratio<u32>,
}

impl Pool {
    pub fn uniswap(address: Address, tokens: TokenPair, reserves: (u128, u128)) -> Self {
        Self {
            address,
            tokens,
            reserves,
            fee: Ratio::new(3, 1000),
        }
    }

    /// Given an input amount and token, returns the maximum output amount and
    /// address of the other asset. Returns None if operation not possible
    /// due to arithmetic issues (e.g. over or underflow)
    fn get_amount_out(&self, token_in: Address, amount_in: U256) -> Option<(U256, Address)> {
        let (reserve_in, reserve_out, token_out) = self.get_relative_reserves(token_in);
        Some((
            self.amount_out(amount_in, reserve_in, reserve_out)?,
            token_out,
        ))
    }

    /// Given an output amount and token, returns a required input amount and
    /// address of the other asset. Returns None if operation not possible
    /// due to arithmetic issues (e.g. over or underflow, reserve too small)
    fn get_amount_in(&self, token_out: Address, amount_out: U256) -> Option<(U256, Address)> {
        let (reserve_out, reserve_in, token_in) = self.get_relative_reserves(token_out);
        Some((
            self.amount_in(amount_out, reserve_in, reserve_out)?,
            token_in,
        ))
    }

    /// Given one of the pool's two tokens, returns a tuple containing the
    /// `RelativeReserves` along with the opposite token. That is, the
    /// elements returned are (respectively)
    /// - the pool's reserve of token provided
    /// - the reserve of the other token
    /// - the pool's other token This is essentially a helper method for
    ///   shuffling values in `get_amount_in` and `get_amount_out`
    fn get_relative_reserves(&self, token: Address) -> RelativeReserves {
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
        let amount_out = numerator.checked_div(denominator)?;

        check_final_reserves(amount_in, amount_out, reserve_in, reserve_out)?;
        Some(amount_out)
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
        let amount_in = numerator.checked_div(denominator)?.checked_add(U256::ONE)?;

        check_final_reserves(amount_in, amount_out, reserve_in, reserve_out)?;
        Some(amount_in)
    }
}

fn check_final_reserves(
    amount_in: U256,
    amount_out: U256,
    reserve_in: U256,
    reserve_out: U256,
) -> Option<(U256, U256)> {
    let final_reserve_in = reserve_in.checked_add(amount_in)?;
    let final_reserve_out = reserve_out.checked_sub(amount_out)?;

    if final_reserve_in > *POOL_MAX_RESERVES {
        None
    } else {
        Some((final_reserve_in, final_reserve_out))
    }
}

impl BaselineSolvable for Pool {
    async fn get_amount_out(
        &self,
        out_token: Address,
        (in_amount, in_token): (U256, Address),
    ) -> Option<U256> {
        self.get_amount_out(in_token, in_amount)
            .map(|(out_amount, token)| {
                assert_eq!(token, out_token);
                out_amount
            })
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        (out_amount, out_token): (U256, Address),
    ) -> Option<U256> {
        self.get_amount_in(out_token, out_amount)
            .map(|(in_amount, token)| {
                assert_eq!(token, in_token);
                in_amount
            })
    }

    async fn gas_cost(&self) -> usize {
        POOL_SWAP_GAS_COST
    }
}

pub struct PoolFetcher<Reader> {
    pub pool_reader: Reader,
    pub web3: Web3,
    pub non_existent_pools: RwLock<TimedCache<TokenPair, ()>>,
}

impl<Reader> PoolFetcher<Reader> {
    pub fn new(reader: Reader, web3: Web3, cache_time: Duration) -> Self {
        Self {
            pool_reader: reader,
            web3,
            non_existent_pools: RwLock::new(TimedCache::with_lifespan(cache_time.as_secs())),
        }
    }
}

#[async_trait::async_trait]
impl<Reader> PoolFetching for PoolFetcher<Reader>
where
    Reader: PoolReading,
{
    #[instrument(skip_all)]
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        let mut futures: FuturesUnordered<_> = {
            let mut non_existent_pools = self.non_existent_pools.write().unwrap();
            token_pairs
                .into_iter()
                .filter(|pair| non_existent_pools.cache_get(pair).is_none())
                .map(|pair| async move {
                    let state = self.pool_reader.read_state(pair, at_block.into()).await?;
                    Ok::<_, anyhow::Error>((pair, state))
                })
                .collect()
        };

        let mut new_missing_pairs = vec![];
        let mut pools = Vec::with_capacity(futures.len());

        while let Some((pair, result)) = futures.try_next().await? {
            match result {
                Some(pool) => pools.push(pool),
                None => new_missing_pairs.push(pair),
            }
        }

        if !new_missing_pairs.is_empty() {
            tracing::debug!(token_pairs = ?new_missing_pairs, "stop indexing liquidity");
            let mut non_existent_pools = self.non_existent_pools.write().unwrap();
            for pair in new_missing_pairs {
                non_existent_pools.cache_set(pair, ());
            }
        }
        Ok(pools)
    }
}

/// The default pool reader implementation.
///
/// This fetches on-chain pool state for Uniswap-like pools assuming a constant
/// fee of 0.3%.
pub struct DefaultPoolReader {
    pub pair_provider: PairProvider,
    pub web3: Web3,
}

impl DefaultPoolReader {
    pub fn new(web3: Web3, pair_provider: PairProvider) -> Self {
        Self {
            pair_provider,
            web3,
        }
    }
}

impl PoolReading for DefaultPoolReader {
    fn read_state(&self, pair: TokenPair, block: BlockId) -> BoxFuture<'_, Result<Option<Pool>>> {
        let pair_address = self.pair_provider.pair_address(&pair);

        // Fetch ERC20 token balances of the pools to sanity check with reserves
        let token0 = ERC20::Instance::new(pair.get().0, self.web3.provider.clone());
        let token1 = ERC20::Instance::new(pair.get().1, self.web3.provider.clone());

        async move {
            let fetch_token0_balance = token0.balanceOf(pair_address).block(block);
            let fetch_token1_balance = token1.balanceOf(pair_address).block(block);

            let pair_contract =
                IUniswapLikePair::Instance::new(pair_address, self.web3.provider.clone());
            let fetch_reserves = pair_contract.getReserves().block(block);

            let (reserves, token0_balance, token1_balance) = futures::join!(
                fetch_reserves.call().into_future(),
                fetch_token0_balance.call().into_future(),
                fetch_token1_balance.call().into_future()
            );

            handle_results(
                FetchedPool {
                    pair,
                    reserves,
                    token0_balance,
                    token1_balance,
                },
                pair_address,
            )
        }
        .boxed()
    }
}

struct FetchedPool {
    pair: TokenPair,
    reserves: Result<getReservesReturn, alloy::contract::Error>,
    token0_balance: Result<U256, alloy::contract::Error>,
    token1_balance: Result<U256, alloy::contract::Error>,
}

fn handle_results(fetched_pool: FetchedPool, address: Address) -> Result<Option<Pool>> {
    let reserves = ignore_non_node_error(fetched_pool.reserves)?;
    let token0_balance = ignore_non_node_error(fetched_pool.token0_balance)?;
    let token1_balance = ignore_non_node_error(fetched_pool.token1_balance)?;

    let pool = reserves.and_then(|reserves| {
        let r0 = u128::try_from(reserves.reserve0).ok()?;
        let r1 = u128::try_from(reserves.reserve1).ok()?;
        // Some ERC20s (e.g. AMPL) have an elastic supply and can thus reduce the
        // balance of their owners without any transfer or other interaction ("rebase").
        // Such behavior can implicitly change the *k* in the pool's constant product
        // formula. E.g. a pool with 10 USDC and 10 AMPL has k = 100. After a negative
        // rebase the pool's AMPL balance may reduce to 9, thus k should be implicitly
        // updated to 90 (figuratively speaking the pool is undercollateralized).
        // Uniswap pools however only update their reserves upon swaps. Such an "out of
        // sync" pool has numerical issues when computing the right clearing price.
        // Note, that a positive rebase is not problematic as k would increase in this
        // case giving the pool excess in the elastic token (an arbitrageur could
        // benefit by withdrawing the excess from the pool without selling anything).
        // We therefore exclude all pools where the pool's token balance of either token
        // in the pair is less than the cached reserve.
        if U256::from(r0) > token0_balance? || U256::from(r1) > token1_balance? {
            return None;
        }
        // Errors here should never happen because reserves are uint<112, 2>
        // meaning they'll always fit in u128, but panicking here is not a good idea
        Some(Pool::uniswap(address, fetched_pool.pair, (r0, r1)))
    });

    Ok(pool)
}

pub mod test_util {
    use {
        super::{Pool, PoolFetching},
        crate::recent_block_cache::Block,
        anyhow::Result,
        model::TokenPair,
        std::collections::HashSet,
    };

    #[derive(Default)]
    pub struct FakePoolFetcher(pub Vec<Pool>);
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
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ethrpc::alloy::errors::{testing_alloy_contract_error, testing_alloy_node_error},
    };

    #[test]
    fn test_get_amounts_out() {
        let sell_token = Address::with_last_byte(1);
        let buy_token = Address::with_last_byte(2);

        // Even Pool
        let pool = Pool::uniswap(
            Address::with_last_byte(1),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (100, 100),
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(10)),
            Some((U256::from(9), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(100)),
            Some((U256::from(49), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(1000)),
            Some((U256::from(90), buy_token))
        );

        //Uneven Pool
        let pool = Pool::uniswap(
            Address::with_last_byte(2),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (200, 50),
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(10)),
            Some((U256::from(2), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(100)),
            Some((U256::from(16), buy_token))
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(1000)),
            Some((U256::from(41), buy_token))
        );

        // Large Numbers
        let pool = Pool::uniswap(
            Address::with_last_byte(3),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (1u128 << 90, 1u128 << 90),
        );
        assert_eq!(
            pool.get_amount_out(sell_token, U256::from(10u128.pow(20))),
            Some((U256::from(99_699_991_970_459_889_807u128), buy_token))
        );

        // Overflow
        assert_eq!(pool.get_amount_out(sell_token, U256::MAX), None);
    }

    #[test]
    fn test_get_amounts_in() {
        let sell_token = Address::with_last_byte(1);
        let buy_token = Address::with_last_byte(2);

        // Even Pool
        let pool = Pool::uniswap(
            Address::with_last_byte(1),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (100, 100),
        );
        assert_eq!(
            pool.get_amount_in(buy_token, U256::from(10)),
            Some((U256::from(12), sell_token))
        );
        assert_eq!(
            pool.get_amount_in(buy_token, U256::from(99)),
            Some((U256::from(9930), sell_token))
        );

        // Buying more than possible
        assert_eq!(pool.get_amount_in(buy_token, U256::from(100)), None);
        assert_eq!(pool.get_amount_in(buy_token, U256::from(1000)), None);

        //Uneven Pool
        let pool = Pool::uniswap(
            Address::with_last_byte(2),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (200, 50),
        );
        assert_eq!(
            pool.get_amount_in(buy_token, U256::from(10)),
            Some((U256::from(51), sell_token))
        );
        assert_eq!(
            pool.get_amount_in(buy_token, U256::from(49)),
            Some((U256::from(9830), sell_token))
        );

        // Large Numbers
        let pool = Pool::uniswap(
            Address::with_last_byte(3),
            TokenPair::new(sell_token, buy_token).unwrap(),
            (1u128 << 90, 1u128 << 90),
        );
        assert_eq!(
            pool.get_amount_in(buy_token, U256::from(10u128.pow(20))),
            Some((U256::from(100_300_910_810_367_424_267u128), sell_token)),
        );
    }

    #[test]
    fn computes_final_reserves() {
        assert_eq!(
            check_final_reserves(
                U256::ONE,
                U256::from(2),
                U256::from(1_000_000),
                U256::from(2_000_000),
            )
            .unwrap(),
            (U256::from(1_000_001), U256::from(1_999_998)),
        );
    }

    #[test]
    fn check_final_reserve_limits() {
        // final out reserve too low
        assert!(
            check_final_reserves(U256::ZERO, U256::ONE, U256::from(1_000_000), U256::ZERO)
                .is_none()
        );
        // final in reserve too high
        assert!(
            check_final_reserves(
                U256::ONE,
                U256::ZERO,
                *POOL_MAX_RESERVES,
                U256::from(1_000_000)
            )
            .is_none()
        );
    }

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let fetched_pool = FetchedPool {
            reserves: Err(testing_alloy_node_error()),
            pair: Default::default(),
            token0_balance: Ok(U256::ONE),
            token1_balance: Ok(U256::ONE),
        };
        let pool_address = Default::default();
        assert!(handle_results(fetched_pool, pool_address).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let fetched_pool = FetchedPool {
            reserves: Err(testing_alloy_contract_error()),
            pair: Default::default(),
            token0_balance: Ok(U256::ONE),
            token1_balance: Ok(U256::ONE),
        };
        let pool_address = Default::default();
        assert!(
            handle_results(fetched_pool, pool_address)
                .unwrap()
                .is_none()
        )
    }
}
