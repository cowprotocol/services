use {
    super::pair_provider::PairProvider,
    crate::{baseline_solvable::BaselineSolvable, recent_block_cache::Block},
    alloy::{
        eips::BlockId,
        primitives::{Address, U256},
        providers::{MulticallItem, Provider},
    },
    anyhow::Result,
    contracts::{
        ERC20,
        IUniswapLikePair::{self, IUniswapLikePair::getReservesReturn},
    },
    ethrpc::Web3,
    futures::{
        FutureExt as _,
        future::{self, BoxFuture},
    },
    model::TokenPair,
    moka::sync::Cache,
    num::rational::Ratio,
    std::{
        collections::HashSet,
        hash::{DefaultHasher, Hash, Hasher},
        sync::LazyLock,
        time::{Duration, Instant},
    },
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

/// Largest multiple of the base delay we back off to. A miss can also mean a
/// pool that exists but is briefly unreadable, so this bounds how long such a
/// pool stays invisible.
const MAX_BACKOFF_FACTOR: u32 = 6;

/// Upper bound on remembered pairs. Only pairs believed to have no pool are
/// held, so this is far above the few tens of thousands seen in practice.
const MAX_REMEMBERED_PAIRS: u64 = 100_000;

/// Pairs nobody asks about any more are forgotten; `max_capacity` bounds the
/// rest.
const FORGET_UNUSED_PAIRS_AFTER: Duration = Duration::from_secs(48 * 3600);

/// A pair we found no pool for, and when to look again.
#[derive(Clone, Copy)]
struct Miss {
    recheck_at: Instant,
    /// How many times in a row we confirmed the pair has no pool.
    confirmations: u32,
}

pub struct PoolFetcher<Reader> {
    pub pool_reader: Reader,
    pub web3: Web3,
    /// Pairs believed to have no pool. Entries outlive their `recheck_at` so
    /// that `confirmations` survives a re-check.
    non_existent_pools: Cache<TokenPair, Miss>,
    /// Delay before the first re-check. Doubles per confirmation.
    base_recheck_delay: Duration,
}

impl<Reader> PoolFetcher<Reader> {
    pub fn new(reader: Reader, web3: Web3, cache_time: Duration) -> Self {
        Self {
            pool_reader: reader,
            web3,
            non_existent_pools: Cache::builder()
                .max_capacity(MAX_REMEMBERED_PAIRS)
                .time_to_idle(FORGET_UNUSED_PAIRS_AFTER)
                .build(),
            base_recheck_delay: cache_time,
        }
    }

    fn needs_check(&self, pair: &TokenPair, now: Instant) -> bool {
        self.non_existent_pools
            .get(pair)
            .is_none_or(|miss| miss.recheck_at <= now)
    }

    /// Records that `pair` has no pool and schedules the next check. Upserts so
    /// that concurrent fetches of the same pair cannot double-count a miss.
    fn record_miss(&self, pair: TokenPair, now: Instant) {
        self.non_existent_pools
            .entry(pair)
            .and_upsert_with(|previous| {
                let confirmations = previous
                    .map_or(0, |entry| entry.into_value().confirmations)
                    .saturating_add(1);
                let delay = self
                    .base_recheck_delay
                    .saturating_mul(backoff_factor(confirmations))
                    .mul_f64(spread_factor(&pair));
                Miss {
                    recheck_at: now + delay,
                    confirmations,
                }
            });
    }
}

/// Doubles per confirmation up to [`MAX_BACKOFF_FACTOR`], so the delays run
/// 1x, 2x, 4x, then 6x the base.
fn backoff_factor(confirmations: u32) -> u32 {
    1u32.checked_shl(confirmations.saturating_sub(1))
        .unwrap_or(MAX_BACKOFF_FACTOR)
        .min(MAX_BACKOFF_FACTOR)
}

/// A per-pair factor in `[1.0, 1.5)` applied to the delay.
///
/// Pairs missed together would otherwise come due together and re-check as one
/// burst. Stable per pair, so the configured delay stays a floor rather than
/// drifting between re-checks.
fn spread_factor(pair: &TokenPair) -> f64 {
    let mut hasher = DefaultHasher::new();
    pair.hash(&mut hasher);
    1.0 + 0.5 * (hasher.finish() as f64 / (u64::MAX as f64 + 1.0))
}

#[async_trait::async_trait]
impl<Reader> PoolFetching for PoolFetcher<Reader>
where
    Reader: PoolReading,
{
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        let now = Instant::now();
        let token_pairs: Vec<_> = token_pairs
            .into_iter()
            .filter(|pair| self.needs_check(pair, now))
            .collect();
        let futures = token_pairs
            .iter()
            .map(|pair| self.pool_reader.read_state(*pair, at_block.into()))
            .collect::<Vec<_>>();

        let results = future::try_join_all(futures).await?;

        let mut new_missing_pairs = vec![];
        let mut pools = vec![];
        for (result, key) in results.into_iter().zip(token_pairs) {
            match result {
                Some(pool) => {
                    // Keeps the cache holding only pairs believed poolless.
                    self.non_existent_pools.invalidate(&key);
                    pools.push(pool);
                }
                None => new_missing_pairs.push(key),
            }
        }
        if !new_missing_pairs.is_empty() {
            tracing::debug!(token_pairs = ?new_missing_pairs, "stop indexing liquidity");
            for pair in new_missing_pairs {
                self.record_miss(pair, now);
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

        let pair_contract =
            IUniswapLikePair::Instance::new(pair_address, self.web3.provider.clone());
        // Fetch ERC20 token balances of the pools to sanity check with reserves
        let token0 = ERC20::Instance::new(pair.get().0, self.web3.provider.clone());
        let token1 = ERC20::Instance::new(pair.get().1, self.web3.provider.clone());

        async move {
            // Every sub-call is allowed to fail on its own: there may be no
            // pool at the address, or a token may not answer
            // `balanceOf`. A failure of the aggregate itself is a
            // node error and propagates for retrying.
            let (reserves, token0_balance, token1_balance) = self
                .web3
                .provider
                .multicall()
                .block(block)
                .add_call(pair_contract.getReserves().into_call(true))
                .add_call(token0.balanceOf(pair_address).into_call(true))
                .add_call(token1.balanceOf(pair_address).into_call(true))
                .aggregate3()
                .await?;

            Ok(handle_results(
                FetchedPool {
                    pair,
                    reserves: reserves.ok(),
                    token0_balance: token0_balance.ok(),
                    token1_balance: token1_balance.ok(),
                },
                pair_address,
            ))
        }
        .boxed()
    }
}

/// Pool state as read from the node. `None` stands for a call that returned no
/// usable data, meaning there is no pool at the address or the token does not
/// answer.
struct FetchedPool {
    pair: TokenPair,
    reserves: Option<getReservesReturn>,
    token0_balance: Option<U256>,
    token1_balance: Option<U256>,
}

fn handle_results(fetched_pool: FetchedPool, address: Address) -> Option<Pool> {
    let FetchedPool {
        pair,
        reserves,
        token0_balance,
        token1_balance,
    } = fetched_pool;

    reserves.and_then(|reserves| {
        let r0 = u128::try_from(reserves.reserve0).ok()?;
        let r1 = u128::try_from(reserves.reserve1).ok()?;
        // Some ERC20s (e.g. AMPL) have an elastic supply and can thus reduce
        // the balance of their owners without any transfer or other
        // interaction ("rebase"). Such behavior can implicitly change
        // the *k* in the pool's constant product formula. E.g. a pool
        // with 10 USDC and 10 AMPL has k = 100. After a negative rebase
        // the pool's AMPL balance may reduce to 9, thus k should be implicitly
        // updated to 90 (figuratively speaking the pool is
        // undercollateralized). Uniswap pools however only update their
        // reserves upon swaps. Such an "out of sync" pool has numerical
        // issues when computing the right clearing price. Note, that a
        // positive rebase is not problematic as k would increase in this
        // case giving the pool excess in the elastic token (an arbitrageur
        // could benefit by withdrawing the excess from the pool without
        // selling anything). We therefore exclude all pools where the
        // pool's token balance of either token in the pair is less than
        // the cached reserve.
        if U256::from(r0) > token0_balance? || U256::from(r1) > token1_balance? {
            return None;
        }
        // Errors here should never happen because reserves are uint<112, 2>
        // meaning they'll always fit in u128, but panicking here is not a good
        // idea
        Some(Pool::uniswap(address, pair, (r0, r1)))
    })
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
        alloy::{
            primitives::{Bytes, aliases::U112},
            providers::{bindings::IMulticall3, mock::Asserter},
            sol_types::SolCall,
        },
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

    fn reserves(reserve0: u128, reserve1: u128) -> getReservesReturn {
        getReservesReturn {
            reserve0: U112::from(reserve0),
            reserve1: U112::from(reserve1),
            blockTimestampLast: 0,
        }
    }

    #[test]
    fn pool_fetcher_skips_pool_without_reserves() {
        let fetched_pool = FetchedPool {
            pair: Default::default(),
            reserves: None,
            token0_balance: Some(U256::ONE),
            token1_balance: Some(U256::ONE),
        };
        assert!(handle_results(fetched_pool, Default::default()).is_none());
    }

    #[test]
    fn pool_fetcher_skips_pool_without_balances() {
        let fetched_pool = FetchedPool {
            pair: Default::default(),
            reserves: Some(reserves(1, 1)),
            token0_balance: Some(U256::ONE),
            token1_balance: None,
        };
        assert!(handle_results(fetched_pool, Default::default()).is_none());
    }

    #[test]
    fn pool_fetcher_keeps_pool_backed_by_its_balances() {
        let fetched_pool = FetchedPool {
            pair: Default::default(),
            reserves: Some(reserves(10, 20)),
            token0_balance: Some(U256::from(10)),
            token1_balance: Some(U256::from(30)),
        };
        let pool = handle_results(fetched_pool, Default::default()).unwrap();
        assert_eq!(pool.reserves, (10, 20));
    }

    /// A negative rebase leaves the pool holding less than its reserves claim,
    /// which makes its clearing price unusable.
    #[test]
    fn pool_fetcher_skips_pool_short_of_its_reserves() {
        let fetched_pool = FetchedPool {
            pair: Default::default(),
            reserves: Some(reserves(10, 20)),
            token0_balance: Some(U256::from(10)),
            token1_balance: Some(U256::from(19)),
        };
        assert!(handle_results(fetched_pool, Default::default()).is_none());
    }

    /// A reader whose node answers with whatever is pushed onto `asserter`.
    fn mocked_reader(asserter: Asserter) -> DefaultPoolReader {
        DefaultPoolReader::new(
            Web3::with_asserter(asserter),
            PairProvider {
                factory: Address::with_last_byte(1),
                init_code_digest: [0; 32],
            },
        )
    }

    fn token_pair() -> TokenPair {
        TokenPair::new(Address::with_last_byte(2), Address::with_last_byte(3)).unwrap()
    }

    /// The `eth_call` response of an `aggregate3` whose sub-calls came back as
    /// given, in the order the reader asked for them.
    fn aggregate3_response(sub_calls: Vec<(bool, Vec<u8>)>) -> Bytes {
        let results = sub_calls
            .into_iter()
            .map(|(success, return_data)| IMulticall3::Result {
                success,
                returnData: return_data.into(),
            })
            .collect();
        IMulticall3::aggregate3Call::abi_encode_returns(&results).into()
    }

    fn encoded_reserves(reserve0: u128, reserve1: u128) -> Vec<u8> {
        IUniswapLikePair::IUniswapLikePair::getReservesCall::abi_encode_returns(&reserves(
            reserve0, reserve1,
        ))
    }

    fn encoded_balance(balance: u128) -> Vec<u8> {
        ERC20::ERC20::balanceOfCall::abi_encode_returns(&U256::from(balance))
    }

    /// A failing node must stay distinguishable from a missing pool, so that
    /// the read is retried instead of the pair being remembered as having
    /// no pool.
    #[tokio::test]
    async fn pool_fetcher_forwards_node_error() {
        let asserter = Asserter::new();
        asserter.push_failure_msg("node is unavailable");

        let reader = mocked_reader(asserter);
        assert!(
            reader
                .read_state(token_pair(), BlockId::latest())
                .await
                .is_err()
        );
    }

    /// Whereas a sub-call the node did answer, but which failed, means there is
    /// nothing to trade against at that address.
    #[tokio::test]
    async fn pool_fetcher_skips_contract_error() {
        let asserter = Asserter::new();
        asserter.push_success(&aggregate3_response(vec![
            (false, vec![]),
            (true, encoded_balance(1)),
            (true, encoded_balance(1)),
        ]));

        let reader = mocked_reader(asserter);
        assert!(
            reader
                .read_state(token_pair(), BlockId::latest())
                .await
                .unwrap()
                .is_none()
        );
    }

    /// A pair that was never deployed is a codeless CREATE2 address.
    /// Calling it succeeds with empty data, which must fail this sub-call
    /// alone, not the whole aggregate.
    #[tokio::test]
    async fn pool_fetcher_skips_codeless_pair_address() {
        let asserter = Asserter::new();
        asserter.push_success(&aggregate3_response(vec![
            (true, vec![]),
            (true, encoded_balance(1)),
            (true, encoded_balance(1)),
        ]));

        let reader = mocked_reader(asserter);
        assert!(
            reader
                .read_state(token_pair(), BlockId::latest())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn pool_fetcher_reads_pool_from_one_aggregated_call() {
        let asserter = Asserter::new();
        asserter.push_success(&aggregate3_response(vec![
            (true, encoded_reserves(10, 20)),
            (true, encoded_balance(10)),
            (true, encoded_balance(20)),
        ]));

        let reader = mocked_reader(asserter);
        let pool = reader
            .read_state(token_pair(), BlockId::latest())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pool.reserves, (10, 20));
        assert_eq!(pool.tokens, token_pair());
    }

    fn fetcher(cache_time: Duration) -> PoolFetcher<DefaultPoolReader> {
        PoolFetcher::new(
            mocked_reader(Asserter::new()),
            Web3::with_asserter(Asserter::new()),
            cache_time,
        )
    }

    #[test]
    fn cached_miss_suppresses_checks_until_due() {
        let base = Duration::from_secs(3600);
        let fetcher = fetcher(base);
        let pair = token_pair();
        let now = Instant::now();

        assert!(fetcher.needs_check(&pair, now), "unknown pair is checked");
        fetcher.record_miss(pair, now);
        assert!(!fetcher.needs_check(&pair, now));
        // The spread factor tops out at 1.5x, so twice the base is always
        // enough to make the pair due again.
        assert!(fetcher.needs_check(&pair, now + base * 2));
    }

    #[test]
    fn backoff_factor_doubles_per_confirmation_up_to_the_cap() {
        for (confirmations, expected) in [(0, 1), (1, 1), (2, 2), (3, 4), (4, 6), (5, 6), (99, 6)] {
            assert_eq!(backoff_factor(confirmations), expected, "{confirmations}");
        }
    }

    #[test]
    fn recheck_delay_grows_and_never_undercuts_the_configured_delay() {
        let base = Duration::from_secs(3600);
        let fetcher = fetcher(base);
        let pair = token_pair();
        let now = Instant::now();

        let mut previous = Duration::ZERO;
        for _ in 1..=8 {
            fetcher.record_miss(pair, now);
            let delay = fetcher.non_existent_pools.get(&pair).unwrap().recheck_at - now;
            assert!(delay >= base, "{delay:?} undercuts the configured delay");
            assert!(delay >= previous, "delay must never shrink");
            previous = delay;
        }
        assert!(
            previous < base * MAX_BACKOFF_FACTOR * 3 / 2,
            "{previous:?} exceeds the cap"
        );
    }

    #[test]
    fn spread_factor_separates_pairs_missed_together() {
        let factors: Vec<_> = (2..202u8)
            .map(|byte| {
                let pair =
                    TokenPair::new(Address::with_last_byte(1), Address::with_last_byte(byte))
                        .unwrap();
                spread_factor(&pair)
            })
            .collect();

        // Spread across a 30 min window at one-second resolution, so ~11 of
        // the 200 collide by birthday paradox. A coarsely quantized factor
        // would collide far more.
        let distinct: HashSet<_> = factors
            .iter()
            .map(|f| (f * 3600.0).round() as u64)
            .collect();
        assert!(
            distinct.len() > 180,
            "only {} distinct delays over 200 pairs",
            distinct.len()
        );
    }

    /// A zero `cache_time` must disable the cache outright, which is the
    /// contract `UniV2BaselineSourceParameters::into_source` builds on.
    #[test]
    fn zero_cache_time_never_suppresses_a_check() {
        let fetcher = fetcher(Duration::ZERO);
        let pair = token_pair();
        let now = Instant::now();

        fetcher.record_miss(pair, now);
        assert!(fetcher.needs_check(&pair, now));
    }

    /// The point of the cache: a remembered miss costs no node call at all.
    /// The asserter is empty, so any `eth_call` would fail the fetch.
    #[tokio::test]
    async fn remembered_miss_issues_no_node_call() {
        let fetcher = fetcher(Duration::from_secs(3600));
        let pair = token_pair();
        fetcher.record_miss(pair, Instant::now());

        let pools = fetcher
            .fetch(HashSet::from([pair]), Block::Recent)
            .await
            .unwrap();
        assert!(pools.is_empty());
    }

    #[tokio::test]
    async fn finding_a_pool_forgets_the_remembered_miss() {
        let asserter = Asserter::new();
        asserter.push_success(&aggregate3_response(vec![
            (true, encoded_reserves(10, 20)),
            (true, encoded_balance(10)),
            (true, encoded_balance(20)),
        ]));
        let fetcher = PoolFetcher::new(
            mocked_reader(asserter),
            Web3::with_asserter(Asserter::new()),
            Duration::ZERO,
        );
        let pair = token_pair();
        fetcher.record_miss(pair, Instant::now());

        let pools = fetcher
            .fetch(HashSet::from([pair]), Block::Recent)
            .await
            .unwrap();
        assert_eq!(pools.len(), 1);
        assert!(fetcher.non_existent_pools.get(&pair).is_none());
    }
}
