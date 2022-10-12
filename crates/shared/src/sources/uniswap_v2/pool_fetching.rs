use super::pair_provider::PairProvider;
use crate::{
    baseline_solver::BaselineSolvable, ethcontract_error::EthcontractErrorType,
    recent_block_cache::Block, transport::MAX_BATCH_SIZE, Web3, Web3CallBatch,
};
use anyhow::Result;
use contracts::{IUniswapLikePair, ERC20};
use ethcontract::{errors::MethodError, BlockId, H160, U256};
use futures::{
    future::{self, BoxFuture},
    FutureExt as _,
};
use model::TokenPair;
use num::rational::Ratio;
use std::collections::HashSet;

const POOL_SWAP_GAS_COST: usize = 60_000;

lazy_static::lazy_static! {
    static ref POOL_MAX_RESERVES: U256 = U256::from((1u128 << 112) - 1);
}

/// This type denotes `(reserve_a, reserve_b, token_b)` where
/// `reserve_a` refers to the reserve of the excluded token.
type RelativeReserves = (U256, U256, H160);

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>>;
}

/// Trait for abstracting the on-chain reading logic for pool state.
pub trait PoolReading: Sized + Send + Sync {
    fn for_pair_provider(pair_provider: PairProvider, web3: Web3) -> Self;

    /// Read the pool state for the specified token pair.
    ///
    /// The caller specifies a Web3 call back to queue RPC requests into as well
    /// as a block number to fetch the data on.
    ///
    /// This method intentionally **does not** use `async_trait` because
    /// implementations are expected to queue up Ethereum RPC calls into the
    /// specified batch when the method is called and not when the resulting
    /// future is first polled.
    fn read_state(
        &self,
        pair: TokenPair,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'_, Result<Option<Pool>>>;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug)]
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
        let amount_in = numerator.checked_div(denominator)?.checked_add(1.into())?;

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
    fn get_amount_out(&self, out_token: H160, (in_amount, in_token): (U256, H160)) -> Option<U256> {
        self.get_amount_out(in_token, in_amount)
            .map(|(out_amount, token)| {
                assert_eq!(token, out_token);
                out_amount
            })
    }

    fn get_amount_in(&self, in_token: H160, (out_amount, out_token): (U256, H160)) -> Option<U256> {
        self.get_amount_in(out_token, out_amount)
            .map(|(in_amount, token)| {
                assert_eq!(token, in_token);
                in_amount
            })
    }

    fn gas_cost(&self) -> usize {
        POOL_SWAP_GAS_COST
    }
}

pub struct PoolFetcher<Reader> {
    pub pool_reader: Reader,
    pub web3: Web3,
}

impl PoolFetcher<DefaultPoolReader> {
    /// Creates a pool fetcher instance for Uniswap V2 (or an exact clone).
    pub fn uniswap(pair_provider: PairProvider, web3: Web3) -> Self {
        Self {
            pool_reader: DefaultPoolReader {
                pair_provider,
                web3: web3.clone(),
            },
            web3,
        }
    }
}

#[async_trait::async_trait]
impl<Reader> PoolFetching for PoolFetcher<Reader>
where
    Reader: PoolReading,
{
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        let mut batch = Web3CallBatch::new(self.web3.transport().clone());
        let block = BlockId::Number(at_block.into());
        let futures = token_pairs
            .into_iter()
            .map(|pair| self.pool_reader.read_state(pair, &mut batch, block))
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        future::join_all(futures)
            .await
            .into_iter()
            .filter_map(|pool| pool.transpose())
            .collect()
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

impl PoolReading for DefaultPoolReader {
    fn for_pair_provider(pair_provider: PairProvider, web3: Web3) -> Self {
        Self {
            pair_provider,
            web3,
        }
    }

    fn read_state(
        &self,
        pair: TokenPair,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'_, Result<Option<Pool>>> {
        let pair_address = self.pair_provider.pair_address(&pair);
        let pair_contract = IUniswapLikePair::at(&self.web3, pair_address);

        // Fetch ERC20 token balances of the pools to sanity check with reserves
        let token0 = ERC20::at(&self.web3, pair.get().0);
        let token1 = ERC20::at(&self.web3, pair.get().1);

        let reserves = pair_contract.get_reserves().block(block).batch_call(batch);
        let token0_balance = token0
            .balance_of(pair_address)
            .block(block)
            .batch_call(batch);
        let token1_balance = token1
            .balance_of(pair_address)
            .block(block)
            .batch_call(batch);

        async move {
            handle_results(FetchedPool {
                pair,
                reserves: reserves.await,
                token0_balance: token0_balance.await,
                token1_balance: token1_balance.await,
            })
        }
        .boxed()
    }
}

struct FetchedPool {
    pair: TokenPair,
    reserves: Result<(u128, u128, u32), MethodError>,
    token0_balance: Result<U256, MethodError>,
    token1_balance: Result<U256, MethodError>,
}

// Node errors should be bubbled up but contract errors should lead to the pool being skipped.
pub fn handle_contract_error<T>(result: Result<T, MethodError>) -> Result<Option<T>> {
    match result {
        Ok(t) => Ok(Some(t)),
        Err(err) => match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Node => Err(err.into()),
            EthcontractErrorType::Contract => Ok(None),
        },
    }
}

fn handle_results(fetched_pool: FetchedPool) -> Result<Option<Pool>> {
    let reserves = handle_contract_error(fetched_pool.reserves)?;
    let token0_balance = handle_contract_error(fetched_pool.token0_balance)?;
    let token1_balance = handle_contract_error(fetched_pool.token1_balance)?;

    let pool = reserves.and_then(|reserves| {
        // Some ERC20s (e.g. AMPL) have an elastic supply and can thus reduce the balance of their owners without any transfer or other interaction ("rebase").
        // Such behavior can implicitly change the *k* in the pool's constant product formula. E.g. a pool with 10 USDC and 10 AMPL has k = 100. After a negative
        // rebase the pool's AMPL balance may reduce to 9, thus k should be implicitly updated to 90 (figuratively speaking the pool is undercollateralized).
        // Uniswap pools however only update their reserves upon swaps. Such an "out of sync" pool has numerical issues when computing the right clearing price.
        // Note, that a positive rebase is not problematic as k would increase in this case giving the pool excess in the elastic token (an arbitrageur could
        // benefit by withdrawing the excess from the pool without selling anything).
        // We therefore exclude all pools where the pool's token balance of either token in the pair is less than the cached reserve.
        if U256::from(reserves.0) > token0_balance? || U256::from(reserves.1) > token1_balance? {
            return None;
        }
        Some(Pool::uniswap(fetched_pool.pair, (reserves.0, reserves.1)))
    });

    Ok(pool)
}

pub mod test_util {
    use std::collections::HashSet;

    use anyhow::Result;
    use model::TokenPair;

    use crate::recent_block_cache::Block;

    use super::{Pool, PoolFetching};

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
    use super::*;
    use crate::ethcontract_error;

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
            (1u128 << 90, 1u128 << 90),
        );
        assert_eq!(
            pool.get_amount_out(sell_token, 10u128.pow(20).into()),
            Some((99_699_991_970_459_889_807u128.into(), buy_token))
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
            (1u128 << 90, 1u128 << 90),
        );
        assert_eq!(
            pool.get_amount_in(buy_token, 10u128.pow(20).into()),
            Some((100_300_910_810_367_424_267u128.into(), sell_token)),
        );
    }

    #[test]
    fn computes_final_reserves() {
        assert_eq!(
            check_final_reserves(1.into(), 2.into(), 1_000_000.into(), 2_000_000.into(),).unwrap(),
            (1_000_001.into(), 1_999_998.into()),
        );
    }

    #[test]
    fn check_final_reserve_limits() {
        // final out reserve too low
        assert!(check_final_reserves(0.into(), 1.into(), 1_000_000.into(), 0.into()).is_none());
        // final in reserve too high
        assert!(
            check_final_reserves(1.into(), 0.into(), *POOL_MAX_RESERVES, 1_000_000.into())
                .is_none()
        );
    }

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let fetched_pool = FetchedPool {
            reserves: Err(ethcontract_error::testing_node_error()),
            pair: Default::default(),
            token0_balance: Ok(1.into()),
            token1_balance: Ok(1.into()),
        };
        assert!(handle_results(fetched_pool).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let fetched_pool = FetchedPool {
            reserves: Err(ethcontract_error::testing_contract_error()),
            pair: Default::default(),
            token0_balance: Ok(1.into()),
            token1_balance: Ok(1.into()),
        };
        assert!(handle_results(fetched_pool).unwrap().is_none())
    }
}
