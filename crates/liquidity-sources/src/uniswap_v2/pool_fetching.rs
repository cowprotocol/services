use {
    super::pair_provider::PairProvider,
    crate::{
        baseline_solvable::BaselineSolvable,
        recent_block_cache::Block,
        uniswap_v2::slotted_expiry::SlottedExpiry,
    },
    alloy::{
        eips::BlockId,
        primitives::{Address, U256},
    },
    anyhow::Result,
    contracts::{
        ERC20,
        IUniswapLikePair::{self, IUniswapLikePair::getReservesReturn},
    },
    ethrpc::{Web3, alloy::errors::ignore_non_node_error},
    futures::{
        FutureExt as _,
        future::{self, BoxFuture},
    },
    model::TokenPair,
    moka::sync::Cache,
    num::rational::Ratio,
    prometheus::{IntCounterVec, IntGaugeVec},
    std::{collections::HashSet, sync::LazyLock, time::Duration},
};

const POOL_SWAP_GAS_COST: usize = 60_000;

/// Upper bound on the token pairs remembered as having no pool.
/// At the time of writing the largest such cache on mainnet holds roughly 20k
/// entries, so this leaves about 5x headroom. Evicting an entry early is not
/// incorrect, but the overflow is then re-probed on every block rather than
/// once per cycle, so the cap wants headroom rather than to be tight.
const MAX_NON_EXISTENT_POOLS: u64 = 100_000;

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Token pairs recorded as having no pool. Every re-probe that re-confirms
    /// a pair counts again, so in steady state this tracks the probe cadence:
    /// roughly one per suppressed pair per `missing_pool_cache_time`.
    ///
    /// Today a transport failure aborts the whole fetch before anything is
    /// recorded, so only per-pair contract-level results land here. Once probes
    /// are batched into a single call, a partially failed batch could record
    /// pairs that do have a pool, and a step change here is where that shows.
    #[metric(labels("venue"))]
    non_existent_pools_cached: IntCounterVec,

    /// Entries resident in the cache, which is what counts against
    /// `MAX_NON_EXISTENT_POOLS`. Slightly ahead of the number of pairs actually
    /// suppressed, because moka keeps an expired entry until its timer fires.
    /// Reaching the cap means new pairs are refused admission and re-probed on
    /// every block rather than once per cycle.
    #[metric(labels("venue"))]
    non_existent_pools_size: IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

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
    /// Identifies this venue in metrics. One process runs a fetcher per
    /// Uniswap-V2-like venue, and they must not share metric series.
    pub venue: String,
    /// Token pairs that returned no usable liquidity. They are suppressed from
    /// being fetched again until their entry expires. `None` when the
    /// configured cache time is zero, which disables suppression: building the
    /// cache anyway would retain entries that expire the moment they land.
    pub non_existent_pools: Option<Cache<TokenPair, ()>>,
}

impl<Reader> PoolFetcher<Reader> {
    pub fn new(reader: Reader, web3: Web3, cache_time: Duration, venue: String) -> Self {
        Self {
            pool_reader: reader,
            web3,
            venue,
            non_existent_pools: (!cache_time.is_zero()).then(|| {
                Cache::builder()
                    .max_capacity(MAX_NON_EXISTENT_POOLS)
                    .expire_after(SlottedExpiry::new(cache_time))
                    .build()
            }),
        }
    }
}

#[async_trait::async_trait]
impl<Reader> PoolFetching for PoolFetcher<Reader>
where
    Reader: PoolReading,
{
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        let metrics = Metrics::get();
        let mut token_pairs: Vec<_> = token_pairs.into_iter().collect();

        if let Some(cache) = &self.non_existent_pools {
            metrics
                .non_existent_pools_size
                .with_label_values(&[&self.venue])
                .set(i64::try_from(cache.entry_count()).unwrap_or(i64::MAX));
            token_pairs.retain(|pair| cache.get(pair).is_none());
        }
        let futures = token_pairs
            .iter()
            .map(|pair| self.pool_reader.read_state(*pair, at_block.into()))
            .collect::<Vec<_>>();

        let results = future::try_join_all(futures).await?;

        let mut new_missing_pairs = vec![];
        let mut pools = vec![];
        for (result, key) in results.into_iter().zip(token_pairs) {
            match result {
                Some(pool) => pools.push(pool),
                None => new_missing_pairs.push(key),
            }
        }
        if !new_missing_pairs.is_empty() {
            tracing::debug!(token_pairs = ?new_missing_pairs, "stop indexing liquidity");
            if let Some(cache) = &self.non_existent_pools {
                metrics
                    .non_existent_pools_cached
                    .with_label_values(&[&self.venue])
                    .inc_by(new_missing_pairs.len() as u64);
                for pair in new_missing_pairs {
                    cache.insert(pair, ());
                }
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
        std::{
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
            time::Instant,
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
    /// A [`PoolReading`] that reports every pair as having no pool and counts
    /// how often it was asked.
    struct CountingReader(Arc<AtomicUsize>);

    impl PoolReading for CountingReader {
        fn read_state(&self, _: TokenPair, _: BlockId) -> BoxFuture<'_, Result<Option<Pool>>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            async { Ok(None) }.boxed()
        }
    }

    fn counting_fetcher(cache_time: Duration) -> (PoolFetcher<CountingReader>, Arc<AtomicUsize>) {
        let reads = Arc::new(AtomicUsize::new(0));
        let fetcher = PoolFetcher::new(
            CountingReader(reads.clone()),
            ethrpc::mock::web3(),
            cache_time,
            "test".to_owned(),
        );
        (fetcher, reads)
    }

    fn a_pair() -> TokenPair {
        TokenPair::new(Address::with_last_byte(1), Address::with_last_byte(2)).unwrap()
    }

    fn some_pair() -> HashSet<TokenPair> {
        HashSet::from([a_pair()])
    }

    fn pair_number(i: u64) -> TokenPair {
        TokenPair::new(
            Address::left_padding_from(&u64::MAX.to_be_bytes()),
            Address::left_padding_from(&i.to_be_bytes()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn non_existent_pool_is_probed_only_once() {
        let (fetcher, reads) = counting_fetcher(Duration::from_secs(60));

        assert!(
            fetcher
                .fetch(some_pair(), Block::Recent)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            fetcher
                .fetch(some_pair(), Block::Recent)
                .await
                .unwrap()
                .is_empty()
        );

        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn non_existent_pool_is_probed_again_after_expiry() {
        let cache_time = Duration::from_millis(50);
        let (fetcher, reads) = counting_fetcher(cache_time);

        fetcher.fetch(some_pair(), Block::Recent).await.unwrap();
        tokio::time::sleep(cache_time * 3).await;
        fetcher.fetch(some_pair(), Block::Recent).await.unwrap();

        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    /// `UniV2BaselineSourceParameters::into_source` builds the fetcher with a
    /// zero cache time, which has to leave the suppression disabled.
    #[tokio::test]
    async fn zero_cache_time_disables_suppression() {
        let (fetcher, reads) = counting_fetcher(Duration::ZERO);

        fetcher.fetch(some_pair(), Block::Recent).await.unwrap();
        fetcher.fetch(some_pair(), Block::Recent).await.unwrap();

        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert!(
            fetcher.non_existent_pools.is_none(),
            "a disabled cache should not be built at all"
        );
    }
    /// A [`PoolReading`] that fails the way a dead node does.
    struct FailingReader;

    impl PoolReading for FailingReader {
        fn read_state(&self, _: TokenPair, _: BlockId) -> BoxFuture<'_, Result<Option<Pool>>> {
            async { Err(anyhow::anyhow!("node is unreachable")) }.boxed()
        }
    }

    /// A node failure must never be mistaken for "this pair has no pool".
    /// Only `handle_results` covered this before, so a move to batched probes
    /// could start caching false negatives without any test noticing. A false
    /// negative silently hides a real pool for a whole cycle.
    #[tokio::test]
    async fn a_node_error_never_records_a_pair_as_missing() {
        let fetcher = PoolFetcher::new(
            FailingReader,
            ethrpc::mock::web3(),
            Duration::from_secs(3600),
            "test".to_owned(),
        );

        assert!(fetcher.fetch(some_pair(), Block::Recent).await.is_err());

        let cache = fetcher.non_existent_pools.as_ref().unwrap();
        cache.run_pending_tasks();
        assert_eq!(cache.entry_count(), 0, "a node error poisoned the cache");
    }

    /// Drives a real `moka` cache to pin the two properties `SlottedExpiry`
    /// exists for. Reverting to `time_to_live` fails the spread assertion;
    /// dropping the `expire_after_update` override fails the cadence one.
    #[tokio::test]
    async fn slotting_keeps_the_cadence_and_spreads_the_re_probes() {
        const BASE: Duration = Duration::from_millis(200);
        const POLL: Duration = Duration::from_millis(10);
        const POLLS: u32 = 60;
        const PAIRS: u64 = 200;

        let (fetcher, reads) = counting_fetcher(BASE);
        let pairs: HashSet<_> = (0..PAIRS).map(pair_number).collect();

        // Discovery is a burst by nature. Measure the re-probes that follow.
        fetcher.fetch(pairs.clone(), Block::Recent).await.unwrap();

        let started = Instant::now();
        let mut previous = reads.load(Ordering::SeqCst);
        let mut per_fetch = Vec::new();
        for _ in 0..POLLS {
            tokio::time::sleep(POLL).await;
            fetcher.fetch(pairs.clone(), Block::Recent).await.unwrap();
            let total = reads.load(Ordering::SeqCst);
            per_fetch.push(total - previous);
            previous = total;
        }

        // One re-probe per pair per cycle. Scale by the time actually spent, so
        // a slow machine shifts the expectation instead of failing the test.
        let cycles = started.elapsed().as_secs_f64() / BASE.as_secs_f64();
        let expected = PAIRS as f64 * cycles;
        let re_probes = per_fetch.iter().sum::<usize>() as f64;

        assert!(
            re_probes < expected * 1.5,
            "{re_probes} re-probes against an expected {expected:.0}: cadence too fast"
        );
        assert!(
            re_probes > expected * 0.5,
            "{re_probes} re-probes against an expected {expected:.0}: cadence too slow"
        );

        // A herd shows up as most fetches seeing nothing and one seeing
        // everything; spread means every fetch sees a few. Deliberately not a
        // bound on the largest fetch, since a scheduling stall legitimately
        // lets one fetch cover a wide slice of the cycle.
        let idle = per_fetch.iter().filter(|probes| **probes == 0).count();
        assert!(
            idle < per_fetch.len() / 4,
            "{idle} of {} fetches re-probed nothing: the herd is synchronised",
            per_fetch.len()
        );
    }
}
