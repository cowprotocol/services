use {
    crate::{baseline_solver::BaselineSolvable, ethrpc::Web3, recent_block_cache::Block}, alloy::{contract::Error as AlloyError, primitives::{Address, Uint}}, anyhow::Result, cached::{Cached, TimedCache}, contracts::{alloy::EulerVault::EulerVault, errors::EthcontractErrorType}, ethcontract::{errors::MethodError, BlockId, H160, U256}, ethrpc::alloy::conversions::IntoAlloy, ethrpc::alloy::conversions::IntoLegacy, futures::{
        future::{self, BoxFuture}, FutureExt as _
    }, model::TokenPair, std::{
        collections::HashSet,
        sync::{LazyLock, RwLock},
        time::Duration,
    }, tracing::instrument
};

const POOL_SWAP_GAS_COST: usize = 60_000;

static POOL_MAX_RESERVES: LazyLock<U256> = LazyLock::new(|| U256::from((1u128 << 112) - 1));

/// This type denotes `(reserve_a, reserve_b, token_b)` where
/// `reserve_a` refers to the reserve of the excluded token.
type RelativeReserves = (U256, U256, H160);

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<DepositContract>>;
}

/// Trait for abstracting the on-chain reading logic for pool state.
pub trait PoolReading: Send + Sync {
    /// Read the pool state for the specified token pair.
    fn read_state(&self, vault: H160, block: BlockId) -> BoxFuture<'_, Result<Option<DepositContract>>>;
}

impl PoolReading for Box<dyn PoolReading> {
    fn read_state(&self, vault: H160, block: BlockId) -> BoxFuture<'_, Result<Option<DepositContract>>> {
        (**self).read_state(vault, block)
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
pub struct DepositContract {
    pub address: H160,
    pub base_token: H160,
    pub share_token: H160,
    pub conversion_rate: U256,
}

impl DepositContract {
    pub fn new(address: H160, base_token: H160, share_token: H160, conversion_rate: U256) -> Self {
        Self {
            address,
            base_token,
            share_token,
            conversion_rate,
        }
    }

    /// Given an input amount and token, returns the maximum output amount and
    /// address of the other asset. Returns None if operation not possible
    /// due to arithmetic issues (e.g. over or underflow)
    fn get_amount_out(&self, token_in: H160, amount_in: U256) -> Option<(U256, H160)> {
        if token_in == self.base_token {
            // TODO: "one" is likely not correct here. where is the "1 ether" function?
            Some((amount_in * U256::one() / self.conversion_rate, self.share_token))
        } else {
            Some((amount_in * self.conversion_rate / U256::one(), self.base_token))
        }
    }

    /// Given an output amount and token, returns a required input amount and
    /// address of the other asset. Returns None if operation not possible
    /// due to arithmetic issues (e.g. over or underflow, reserve too small)
    fn get_amount_in(&self, token_out: H160, amount_out: U256) -> Option<(U256, H160)> {
        if token_out == self.base_token {
            Some((amount_out * U256::one() / self.conversion_rate, self.share_token))
        } else {
            Some((amount_out * self.conversion_rate / U256::one(), self.base_token))
        }
    }
}

impl BaselineSolvable for DepositContract {
    async fn get_amount_out(
        &self,
        out_token: H160,
        (in_amount, in_token): (U256, H160),
    ) -> Option<U256> {
        self.get_amount_out(in_token, in_amount)
            .map(|(out_amount, token)| {
                assert_eq!(token, out_token);
                out_amount
            })
    }

    async fn get_amount_in(
        &self,
        in_token: H160,
        (out_amount, out_token): (U256, H160),
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
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<DepositContract>> {
        let mut token_pairs: Vec<_> = token_pairs.into_iter().collect();
        {
            let mut non_existent_pools = self.non_existent_pools.write().unwrap();
            token_pairs.retain(|pair| non_existent_pools.cache_get(pair).is_none());
        }
        let block = BlockId::Number(at_block.into());
        let futures = token_pairs
            .iter()
            .map(|pair| self.pool_reader.read_state(pair[0], block))
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
pub struct DefaultDepositContractReader {
    pub web3: Web3,
}

impl DefaultDepositContractReader {
    pub fn new(web3: Web3) -> Self {
        Self {
            web3,
        }
    }
}

impl PoolReading for DefaultDepositContractReader {
    fn read_state(&self, vault: H160, block: BlockId) -> BoxFuture<'_, Result<Option<DepositContract>>> {

        // TODO: I am having a lot of difficulty with the alloy provider not playing nice with the
        // legacy `Web3` so I return dummy data here for now
        async move {
            handle_results(FetchedDepositContract { asset: Ok(vault), conversion_rate: Ok(Uint::ONE) }, vault)
        }.boxed()
        /*let vault_contract = EulerVault::new(vault.into_alloy(), &self.web3);
        let fetch_asset = vault_contract.asset().block(block.into_alloy()).call();

        let fetch_conversion_rate = vault_contract.convertToAssets(Uint::ONE).block(block.into_alloy()).call();

        async move {
            let (asset, conversion_rate) =
                futures::join!(fetch_asset.into_future(), fetch_conversion_rate.into_future());
            handle_results(
                FetchedDepositContract {
                    asset,
                    conversion_rate,
                },
                vault,
            )
        }
        .boxed()*/
    }
}

struct FetchedDepositContract {
    asset: Result<Address, AlloyError>,
    conversion_rate: Result<Uint<256, 4>, AlloyError>,
}

// Node errors should be bubbled up but contract errors should lead to the pool
// being skipped.
pub fn handle_contract_error<T>(result: Result<T, AlloyError>) -> Result<T> {
    match result {
        Ok(t) => Ok(t),
        Err(err) => Err(err.into()),
    }
}

fn handle_results(fetched_pool: FetchedDepositContract, address: H160) -> Result<Option<DepositContract>> {
    let base_token = handle_contract_error(fetched_pool.asset)?;
    let conversion_rate = handle_contract_error(fetched_pool.conversion_rate)?;

    let dc = Some(DepositContract::new(
        address,
        base_token.into_legacy(),
        address,
        U256(conversion_rate.into_limbs()),
    ));

    Ok(dc)
}
