//! Module with data types and logic common to multiple Balancer pool types

use {
    super::{FactoryIndexing, Pool, PoolIndexing as _, PoolStatus},
    crate::{
        sources::balancer_v2::{
            graph_api::{PoolData, PoolType},
            swap::fixed_point::Bfp,
        },
        token_info::TokenInfoFetching,
    },
    alloy::{
        eips::BlockId,
        primitives::{Address, B256, U256},
    },
    anyhow::{Context, Result, anyhow, ensure},
    contracts::alloy::{BalancerV2BasePool, BalancerV2Vault},
    futures::{FutureExt as _, future::BoxFuture},
    std::{collections::BTreeMap, future::Future, sync::Arc},
    tokio::sync::oneshot,
};

/// Trait for fetching pool data that is generic on a factory type.
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait::async_trait]
pub trait PoolInfoFetching<Factory>: Send + Sync
where
    Factory: FactoryIndexing,
{
    async fn fetch_pool_info(
        &self,
        pool_address: Address,
        block_created: u64,
    ) -> Result<Factory::PoolInfo>;

    fn fetch_pool(
        &self,
        pool: &Factory::PoolInfo,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>>;
}

/// Generic pool info fetcher for fetching pool info and state that is generic
/// on a pool factory type and its inner pool type.
pub struct PoolInfoFetcher<Factory> {
    vault: BalancerV2Vault::Instance,
    factory: Factory,
    token_infos: Arc<dyn TokenInfoFetching>,
}

impl<Factory> PoolInfoFetcher<Factory> {
    pub fn new(
        vault: BalancerV2Vault::Instance,
        factory: Factory,
        token_infos: Arc<dyn TokenInfoFetching>,
    ) -> Self {
        Self {
            vault,
            factory,
            token_infos,
        }
    }

    /// Returns a Balancer base pool contract instance at the specified address.
    fn base_pool_at(&self, pool_address: Address) -> BalancerV2BasePool::Instance {
        let provider = self.vault.provider().clone();
        BalancerV2BasePool::Instance::new(pool_address, provider)
    }

    /// Retrieves the scaling exponents for the specified tokens.
    async fn scaling_factors(&self, tokens: &[Address]) -> Result<Vec<Bfp>> {
        let token_infos = self.token_infos.get_token_infos(tokens).await;
        tokens
            .iter()
            .map(|token| {
                let decimals = token_infos
                    .get(token)
                    .ok_or_else(|| anyhow!("missing token info for {:?}", token))?
                    .decimals
                    .ok_or_else(|| anyhow!("missing decimals for token {:?}", token))?;
                scaling_factor_from_decimals(decimals)
            })
            .collect()
    }

    async fn fetch_common_pool_info(
        &self,
        pool_address: Address,
        block_created: u64,
    ) -> Result<PoolInfo> {
        let pool = self.base_pool_at(pool_address);

        let pool_id = pool.getPoolId().call().await?;
        let tokens = self
            .vault
            .getPoolTokens(pool_id.0.into())
            .call()
            .await?
            .tokens;
        let scaling_factors = self.scaling_factors(&tokens).await?;

        Ok(PoolInfo {
            id: pool_id,
            address: pool_address,
            tokens,
            scaling_factors,
            block_created,
        })
    }

    fn fetch_common_pool_state(
        &self,
        pool: &PoolInfo,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolState>> {
        let pool_address = pool.address;
        let pool_id = pool.id;
        let vault = self.vault.clone();
        let pool_contract_paused = self.base_pool_at(pool_address);
        let pool_contract_fee = self.base_pool_at(pool_address);

        let fetch_paused = async move {
            pool_contract_paused
                .getPausedState()
                .block(block)
                .call()
                .await
                .map(|result| result.paused)
        };
        let fetch_swap_fee = async move {
            pool_contract_fee
                .getSwapFeePercentage()
                .block(block)
                .call()
                .await
        };
        let pool_tokens = async move {
            vault
                .getPoolTokens(pool_id.0.into())
                .block(block)
                .call()
                .await
        };

        // Because of a `mockall` limitation, we **need** the future returned
        // here to be `'static`. This requires us to clone and move `pool` into
        // the async closure - otherwise it would only live for as long as
        // `pool`, i.e. `'_`.
        let pool = pool.clone();
        async move {
            let (paused, swap_fee, pool_tokens) =
                futures::try_join!(fetch_paused, fetch_swap_fee, pool_tokens)?;
            let swap_fee = Bfp::from_wei(swap_fee);

            let balances = pool_tokens.balances;
            let tokens = pool_tokens.tokens.into_iter().collect::<Vec<_>>();
            ensure!(pool.tokens == tokens, "pool token mismatch");
            let tokens = itertools::izip!(&pool.tokens, balances, &pool.scaling_factors)
                .map(|(&address, balance, &scaling_factor)| {
                    (
                        address,
                        TokenState {
                            balance,
                            scaling_factor,
                        },
                    )
                })
                .collect();

            Ok(PoolState {
                paused,
                swap_fee,
                tokens,
            })
        }
        .boxed()
    }
}

#[async_trait::async_trait]
impl<Factory> PoolInfoFetching<Factory> for PoolInfoFetcher<Factory>
where
    Factory: FactoryIndexing,
{
    async fn fetch_pool_info(
        &self,
        pool_address: Address,
        block_created: u64,
    ) -> Result<Factory::PoolInfo> {
        let common_pool_info = self
            .fetch_common_pool_info(pool_address, block_created)
            .await?;
        self.factory.specialize_pool_info(common_pool_info).await
    }

    fn fetch_pool(
        &self,
        pool_info: &Factory::PoolInfo,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>> {
        let pool_id = pool_info.common().id;
        let (common_pool_state, common_pool_state_ok) =
            share_common_pool_state(self.fetch_common_pool_state(pool_info.common(), block));
        let pool_state =
            self.factory
                .fetch_pool_state(pool_info, common_pool_state_ok.boxed(), block);

        async move {
            let common_pool_state = common_pool_state.await?;
            if common_pool_state.paused {
                return Ok(PoolStatus::Paused);
            }
            let pool_state = match pool_state.await? {
                Some(state) => state,
                None => return Ok(PoolStatus::Disabled),
            };

            Ok(PoolStatus::Active(Pool {
                id: pool_id,
                kind: pool_state.into(),
            }))
        }
        .boxed()
    }
}

/// Common pool data shared across all Balancer pools.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub id: B256,
    pub address: Address,
    pub tokens: Vec<Address>,
    pub scaling_factors: Vec<Bfp>,
    pub block_created: u64,
}

impl PoolInfo {
    /// Loads a pool info from Graph pool data.
    pub fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        ensure!(pool.tokens.len() > 1, "insufficient tokens in pool");

        Ok(PoolInfo {
            id: pool.id,
            address: pool.address,
            tokens: pool.tokens.iter().map(|token| token.address).collect(),
            scaling_factors: pool
                .tokens
                .iter()
                .map(|token| scaling_factor_from_decimals(token.decimals))
                .collect::<Result<_>>()?,
            block_created,
        })
    }

    /// Loads a common pool info from Graph pool data, requiring the pool type
    /// to be the specified value.
    pub fn for_type(pool_type: PoolType, pool: &PoolData, block_created: u64) -> Result<Self> {
        ensure!(
            pool.pool_type == pool_type,
            "cannot convert {:?} pool to {:?} pool",
            pool.pool_type,
            pool_type,
        );
        Self::from_graph_data(pool, block_created)
    }
}

/// Common pool state information shared across all pool types.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolState {
    pub paused: bool,
    pub swap_fee: Bfp,
    pub tokens: BTreeMap<Address, TokenState>,
}

/// Common pool token state information that is shared among all pool types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub balance: U256,
    pub scaling_factor: Bfp,
}

/// Compute the scaling rate from a Balancer pool's scaling factor.
///
/// A "scaling rate" is what the optimisation solvers (a.k.a. Quasimodo) expects
/// for token scaling, specifically, it expects a `double` that, when dividing
/// a token amount, would return its amount in base units:
///
/// ```text
///     auto in = in_unscaled / m_scaling_rates.at(t_in).convert_to<double>();
/// ```
///
/// In other words, this is the **inverse** of the scaling factor, as it is
/// defined in the Balancer V2 math.
pub fn compute_scaling_rate(scaling_factor: Bfp) -> Result<U256> {
    Bfp::exp10(18)
        .as_uint256()
        .checked_div(scaling_factor.as_uint256())
        .context("unsupported scaling factor of 0")
}

/// Converts a token decimal count to its corresponding scaling factor.
pub fn scaling_factor_from_decimals(decimals: u8) -> Result<Bfp> {
    Ok(Bfp::exp10(scaling_exponent_from_decimals(decimals)? as _))
}

/// Converts a token decimal count to its corresponding scaling exponent.
pub fn scaling_exponent_from_decimals(decimals: u8) -> Result<u8> {
    // Technically this should never fail for Balancer Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts)
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/deployments-latest/pkg/pool-utils/contracts/BasePool.sol#L476-L487
    18u8.checked_sub(decimals)
        .context("unsupported token with more than 18 decimals")
}

/// An internal utility method for sharing the success value for an
/// `anyhow::Result`.
///
/// Typically, this is pretty trivial using `FutureExt::shared`. However, since
/// `anyhow::Error: !Clone` we need to use a different approach.
///
/// # Panics
///
/// Polling the future with the shared success value will panic if the result
/// future has not already resolved to a `Ok` value. This method is only ever
/// meant to be used internally, so we don't have to worry that these
/// assumptions leak out of this module.
fn share_common_pool_state(
    fut: impl Future<Output = Result<PoolState>>,
) -> (
    impl Future<Output = Result<PoolState>>,
    impl Future<Output = PoolState>,
) {
    let (pool_sender, mut pool_receiver) = oneshot::channel();

    let result = fut.inspect(|pool_result| {
        let pool_result = match pool_result {
            Ok(pool) => Ok(pool.clone()),
            // We can't clone `anyhow::Error` so just use an empty `()` error.
            Err(_) => Err(()),
        };
        // Ignore error if the shared future was dropped.
        let _ = pool_sender.send(pool_result);
    });
    let shared = async move {
        pool_receiver
            .try_recv()
            .expect("result future is still pending or has been dropped")
            .expect("result future resolved to an error")
    };

    (result, shared)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            sources::balancer_v2::{
                PoolKind,
                graph_api::{PoolType, Token},
                pools::{MockFactoryIndexing, weighted},
            },
            token_info::{MockTokenInfoFetching, TokenInfo},
        },
        alloy::{
            providers::{Provider, ProviderBuilder},
            sol_types::SolCall,
            transports::mock::Asserter,
        },
        anyhow::bail,
        contracts::alloy::BalancerV2Vault,
        maplit::{btreemap, hashmap},
        mockall::predicate,
        std::future,
    };

    #[tokio::test]
    async fn fetch_common_pool_info() {
        let pool_id = alloy::primitives::FixedBytes([0x90; 32]);
        let tokens = [
            Address::repeat_byte(1),
            Address::repeat_byte(2),
            Address::repeat_byte(3),
        ];

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());
        asserter.push_success(&pool_id);

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: tokens.to_vec(),
                    balances: vec![],
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let mut token_infos = MockTokenInfoFetching::new();
        token_infos
            .expect_get_token_infos()
            .withf(move |t| t == tokens)
            .returning(move |_| {
                hashmap! {
                    tokens[0] => TokenInfo { decimals: Some(18), symbol: None },
                    tokens[1] => TokenInfo { decimals: Some(18), symbol: None },
                    tokens[2] => TokenInfo { decimals: Some(6), symbol: None },
                }
            });

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = pool_info_fetcher
            .fetch_common_pool_info(*pool.address(), 1337)
            .await
            .unwrap();

        assert_eq!(
            pool_info,
            PoolInfo {
                id: pool_id,
                address: *pool.address(),
                tokens: tokens.to_vec(),
                scaling_factors: vec![Bfp::exp10(0), Bfp::exp10(0), Bfp::exp10(12)],
                block_created: 1337,
            }
        );
    }

    #[tokio::test]
    async fn fetch_common_pool_state() {
        let pool_id = B256::repeat_byte(0x90);
        let tokens = [
            Address::repeat_byte(1),
            Address::repeat_byte(2),
            Address::repeat_byte(3),
        ];
        let balances = [bfp!("1000.0"), bfp!("10.0"), bfp!("15.0")];
        let scaling_factors = [Bfp::exp10(0), Bfp::exp10(0), Bfp::exp10(12)];

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());

        let get_paused_state_response =
            BalancerV2BasePool::BalancerV2BasePool::getPausedStateCall::abi_encode_returns(
                &BalancerV2BasePool::BalancerV2BasePool::getPausedStateReturn {
                    paused: false,
                    pauseWindowEndTime: U256::ZERO,
                    bufferPeriodEndTime: U256::ZERO,
                },
            );
        asserter.push_success(&get_paused_state_response);
        let get_swap_fee_percentage_response =
            BalancerV2BasePool::BalancerV2BasePool::getSwapFeePercentageCall::abi_encode_returns(
                &bfp!("0.003").as_uint256(),
            );
        asserter.push_success(&get_swap_fee_percentage_response);

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: tokens.to_vec(),
                    balances: balances.iter().map(|b| b.as_uint256()).collect(),
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let token_infos = MockTokenInfoFetching::new();

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = PoolInfo {
            id: pool_id,
            address: *pool.address(),
            tokens: tokens.to_vec(),
            scaling_factors: scaling_factors.to_vec(),
            block_created: 1337,
        };

        let pool_state = {
            let pool_state =
                pool_info_fetcher.fetch_common_pool_state(&pool_info, BlockId::Number(1.into()));

            pool_state.await.unwrap()
        };

        assert_eq!(
            pool_state,
            PoolState {
                paused: false,
                swap_fee: bfp!("0.003"),
                tokens: btreemap! {
                    tokens[0] => TokenState {
                        balance: balances[0].as_uint256(),
                        scaling_factor: scaling_factors[0],
                    },
                    tokens[1] => TokenState {
                        balance: balances[1].as_uint256(),
                        scaling_factor: scaling_factors[1],
                    },
                    tokens[2] => TokenState {
                        balance: balances[2].as_uint256(),
                        scaling_factor: scaling_factors[2],
                    },
                },
            }
        );
    }

    #[tokio::test]
    async fn fetch_state_errors_on_token_mismatch() {
        let tokens = [
            Address::repeat_byte(1),
            Address::repeat_byte(2),
            Address::repeat_byte(3),
        ];

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());

        let get_paused_state_response =
            BalancerV2BasePool::BalancerV2BasePool::getPausedStateCall::abi_encode_returns(
                &BalancerV2BasePool::BalancerV2BasePool::getPausedStateReturn {
                    paused: false,
                    pauseWindowEndTime: U256::ZERO,
                    bufferPeriodEndTime: U256::ZERO,
                },
            );
        asserter.push_success(&get_paused_state_response);

        let get_swap_fee_percentage_response =
            BalancerV2BasePool::BalancerV2BasePool::getSwapFeePercentageCall::abi_encode_returns(
                &U256::ZERO,
            );
        asserter.push_success(&get_swap_fee_percentage_response);

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: vec![Address::repeat_byte(1), Address::repeat_byte(4)],
                    balances: vec![U256::ZERO, U256::ZERO],
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let token_infos = MockTokenInfoFetching::new();

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = PoolInfo {
            id: Default::default(),
            address: *pool.address(),
            tokens: tokens.to_vec(),
            scaling_factors: vec![Bfp::exp10(0), Bfp::exp10(0), Bfp::exp10(0)],
            block_created: 1337,
        };

        let pool_state = {
            let pool_state =
                pool_info_fetcher.fetch_common_pool_state(&pool_info, BlockId::Number(1.into()));

            pool_state.await
        };

        assert!(pool_state.is_err());
    }

    #[tokio::test]
    async fn fetch_specialized_pool_state() {
        let swap_fee = bfp!("0.003");

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());

        let get_paused_state_response =
            BalancerV2BasePool::BalancerV2BasePool::getPausedStateCall::abi_encode_returns(
                &BalancerV2BasePool::BalancerV2BasePool::getPausedStateReturn {
                    paused: false,
                    pauseWindowEndTime: U256::ZERO,
                    bufferPeriodEndTime: U256::ZERO,
                },
            );
        asserter.push_success(&get_paused_state_response);
        let get_swap_fee_percentage_response =
            BalancerV2BasePool::BalancerV2BasePool::getSwapFeePercentageCall::abi_encode_returns(
                &swap_fee.as_uint256(),
            );
        asserter.push_success(&get_swap_fee_percentage_response);

        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: B256::repeat_byte(0x90),
                address: *pool.address(),
                tokens: vec![
                    Address::repeat_byte(1),
                    Address::repeat_byte(2),
                    Address::repeat_byte(3),
                ],
                scaling_factors: vec![Bfp::exp10(0), Bfp::exp10(0), Bfp::exp10(12)],
                block_created: 1337,
            },
            weights: vec![bfp!("0.5"), bfp!("0.25"), bfp!("0.25")],
        };
        let pool_state = weighted::PoolState {
            swap_fee,
            tokens: btreemap! {
                pool_info.common.tokens[0] => weighted::TokenState {
                    common: TokenState {
                        balance: bfp!("1000.0").as_uint256(),
                        scaling_factor: pool_info.common.scaling_factors[0],
                    },
                    weight: pool_info.weights[0],
                },
                pool_info.common.tokens[1] => weighted::TokenState {
                    common: TokenState {
                        balance: bfp!("10.0").as_uint256(),
                        scaling_factor: pool_info.common.scaling_factors[1],
                    },
                    weight: pool_info.weights[1],
                },
                pool_info.common.tokens[2] => weighted::TokenState {
                    common: TokenState {
                        balance: bfp!("15.0").as_uint256(),
                        scaling_factor: pool_info.common.scaling_factors[2],
                    },
                    weight: pool_info.weights[2],
                },
            },
            version: Default::default(),
        };

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: pool_info.common.tokens.clone(),
                    balances: pool_state
                        .tokens
                        .values()
                        .map(|token| token.common.balance)
                        .collect(),
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let mut factory = MockFactoryIndexing::new();
        let block_id = BlockId::Number(1.into());
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::eq(pool_info.clone()),
                predicate::always(),
                predicate::eq(block_id),
            )
            .returning({
                let pool_state = pool_state.clone();
                move |_, _, _| future::ready(Ok(Some(pool_state.clone()))).boxed()
            });

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };

        let pool_status = pool_info_fetcher
            .fetch_pool(&pool_info, block_id)
            .await
            .unwrap();

        assert_eq!(
            pool_status,
            PoolStatus::Active(Pool {
                id: pool_info.common.id,
                kind: PoolKind::Weighted(pool_state),
            })
        );
    }

    #[tokio::test]
    async fn fetch_specialized_pool_state_for_paused_pool() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());

        let get_paused_state_response =
            BalancerV2BasePool::BalancerV2BasePool::getPausedStateCall::abi_encode_returns(
                &BalancerV2BasePool::BalancerV2BasePool::getPausedStateReturn {
                    paused: true,
                    pauseWindowEndTime: U256::ZERO,
                    bufferPeriodEndTime: U256::ZERO,
                },
            );
        asserter.push_success(&get_paused_state_response);

        let get_swap_fee_percentage_response =
            BalancerV2BasePool::BalancerV2BasePool::getSwapFeePercentageCall::abi_encode_returns(
                &U256::ZERO,
            );
        asserter.push_success(&get_swap_fee_percentage_response);

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: vec![],
                    balances: vec![],
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let mut factory = MockFactoryIndexing::new();
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::always(),
                predicate::always(),
                predicate::always(),
            )
            .returning(|_, _, _| {
                future::ready(Ok(Some(weighted::PoolState {
                    swap_fee: Bfp::zero(),
                    tokens: Default::default(),
                    version: Default::default(),
                })))
                .boxed()
            });

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };
        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: Default::default(),
                address: *pool.address(),
                tokens: Default::default(),
                scaling_factors: Default::default(),
                block_created: Default::default(),
            },
            weights: Default::default(),
        };

        let pool_status = {
            pool_info_fetcher
                .fetch_pool(&pool_info, BlockId::Number(1.into()))
                .await
                .unwrap()
        };

        assert_eq!(pool_status, PoolStatus::Paused);
    }

    #[tokio::test]
    async fn fetch_specialized_pool_state_for_disabled_pool() {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool = BalancerV2BasePool::Instance::new(Address::random(), provider.clone());
        let vault = BalancerV2Vault::Instance::new(Address::random(), provider.clone());

        let get_paused_state_response =
            BalancerV2BasePool::BalancerV2BasePool::getPausedStateCall::abi_encode_returns(
                &BalancerV2BasePool::BalancerV2BasePool::getPausedStateReturn {
                    paused: false,
                    pauseWindowEndTime: U256::ZERO,
                    bufferPeriodEndTime: U256::ZERO,
                },
            );
        asserter.push_success(&get_paused_state_response);

        let get_swap_fee_percentage_response =
            BalancerV2BasePool::BalancerV2BasePool::getSwapFeePercentageCall::abi_encode_returns(
                &U256::ZERO,
            );
        asserter.push_success(&get_swap_fee_percentage_response);

        let get_pool_tokens_response =
            BalancerV2Vault::BalancerV2Vault::getPoolTokensCall::abi_encode_returns(
                &BalancerV2Vault::BalancerV2Vault::getPoolTokensReturn {
                    tokens: vec![],
                    balances: vec![],
                    lastChangeBlock: U256::ZERO,
                },
            );
        asserter.push_success(&get_pool_tokens_response);

        let mut factory = MockFactoryIndexing::new();
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::always(),
                predicate::always(),
                predicate::always(),
            )
            .returning(|_, _, _| future::ready(Ok(None)).boxed());

        let pool_info_fetcher = PoolInfoFetcher {
            vault,
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };
        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: Default::default(),
                address: *pool.address(),
                tokens: Default::default(),
                scaling_factors: Default::default(),
                block_created: Default::default(),
            },
            weights: Default::default(),
        };

        let pool_status = {
            pool_info_fetcher
                .fetch_pool(&pool_info, BlockId::Number(1.into()))
                .await
                .unwrap()
        };

        assert_eq!(pool_status, PoolStatus::Disabled);
    }

    #[tokio::test]
    async fn scaling_factor_error_on_missing_info() {
        let mut token_infos = MockTokenInfoFetching::new();
        token_infos
            .expect_get_token_infos()
            .returning(|_| hashmap! {});

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::Instance::new(
                Address::repeat_byte(0xba),
                ethrpc::mock::web3().alloy,
            ),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        assert!(
            pool_info_fetcher
                .scaling_factors(&[Address::repeat_byte(0xff)])
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn scaling_factor_error_on_missing_decimals() {
        let token = Address::repeat_byte(0xff);
        let mut token_infos = MockTokenInfoFetching::new();
        token_infos.expect_get_token_infos().returning(move |_| {
            hashmap! {
                token => TokenInfo { decimals: None, symbol: None },
            }
        });

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::Instance::new(
                Address::repeat_byte(0xba),
                ethrpc::mock::web3().alloy,
            ),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        assert!(pool_info_fetcher.scaling_factors(&[token]).await.is_err());
    }

    #[test]
    fn convert_graph_pool_to_common_pool_info() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: B256::repeat_byte(4),
            address: Address::repeat_byte(3),
            factory: Address::repeat_byte(0xfb),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: Address::repeat_byte(0x33),
                    decimals: 3,
                    weight: None,
                },
                Token {
                    address: Address::repeat_byte(0x44),
                    decimals: 18,
                    weight: None,
                },
            ],
        };

        assert_eq!(
            PoolInfo::from_graph_data(&pool, 42).unwrap(),
            PoolInfo {
                id: B256::repeat_byte(4),
                address: Address::repeat_byte(3),
                tokens: vec![Address::repeat_byte(0x33), Address::repeat_byte(0x44)],
                scaling_factors: vec![Bfp::exp10(15), Bfp::exp10(0)],
                block_created: 42,
            }
        );
    }

    #[test]
    fn pool_conversion_insufficient_tokens() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: B256::repeat_byte(2),
            address: Address::repeat_byte(1),
            factory: Address::repeat_byte(0),
            swap_enabled: true,
            tokens: vec![Token {
                address: Address::repeat_byte(2),
                decimals: 18,
                weight: Some("1.337".parse().unwrap()),
            }],
        };
        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[test]
    fn pool_conversion_invalid_decimals() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: B256::repeat_byte(2),
            address: Address::repeat_byte(1),
            factory: Address::repeat_byte(0),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: Address::repeat_byte(2),
                    decimals: 19,
                    weight: Some("1.337".parse().unwrap()),
                },
                Token {
                    address: Address::repeat_byte(3),
                    decimals: 18,
                    weight: Some("1.337".parse().unwrap()),
                },
            ],
        };
        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[test]
    fn scaling_factor_from_decimals_ok_and_err() {
        for i in 0_u8..=18 {
            assert_eq!(
                scaling_factor_from_decimals(i).unwrap(),
                Bfp::exp10(18 - i as i32)
            );
        }
        assert_eq!(
            scaling_factor_from_decimals(19).unwrap_err().to_string(),
            "unsupported token with more than 18 decimals"
        )
    }

    #[tokio::test]
    async fn share_pool_state_future() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { Ok(Default::default()) });
        assert_eq!({ pool_state.await.unwrap() }, pool_state_ok.await);
    }

    #[tokio::test]
    #[should_panic]
    async fn shared_pool_state_future_panics_if_pending() {
        let (_pool_state, pool_state_ok) = share_common_pool_state(async {
            futures::pending!();
            Ok(Default::default())
        });
        pool_state_ok.await;
    }

    #[tokio::test]
    #[should_panic]
    async fn share_pool_state_future_if_dropped() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { Ok(Default::default()) });
        drop(pool_state);
        pool_state_ok.await;
    }

    #[tokio::test]
    #[should_panic]
    async fn share_pool_state_future_if_errored() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { bail!("error") });
        let _ = pool_state.await;
        pool_state_ok.await;
    }

    #[test]
    fn compute_scaling_rates() {
        assert_eq!(
            compute_scaling_rate(scaling_factor_from_decimals(18).unwrap()).unwrap(),
            U256::from(1_000_000_000_000_000_000_u128),
        );
        assert_eq!(
            compute_scaling_rate(scaling_factor_from_decimals(6).unwrap()).unwrap(),
            U256::from(1_000_000)
        );
        assert_eq!(
            compute_scaling_rate(scaling_factor_from_decimals(0).unwrap()).unwrap(),
            U256::from(1)
        );
    }
}
