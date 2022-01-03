//! Module with data types and logic common to multiple Balancer pool types

use super::{FactoryIndexing, Pool, PoolIndexing as _, PoolStatus};
use crate::{
    sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    token_info::TokenInfoFetching,
    Web3CallBatch,
};
use anyhow::{anyhow, ensure, Result};
use contracts::{BalancerV2BasePool, BalancerV2Vault};
use ethcontract::{BlockId, Bytes, H160, H256, U256};
use futures::{future::BoxFuture, FutureExt as _};
use std::{collections::BTreeMap, future::Future, sync::Arc};
use tokio::sync::oneshot;

/// Trait for fetching pool data that is generic on a factory type.
#[mockall::automock]
#[async_trait::async_trait]
pub trait PoolInfoFetching<Factory>: Send + Sync
where
    Factory: FactoryIndexing,
{
    async fn fetch_pool_info(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<Factory::PoolInfo>;

    fn fetch_pool(
        &self,
        pool: &Factory::PoolInfo,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>>;
}

/// Generic pool info fetcher for fetching pool info and state that is generic
/// on a pool factory type and its inner pool type.
pub struct PoolInfoFetcher<Factory> {
    vault: BalancerV2Vault,
    factory: Factory,
    token_infos: Arc<dyn TokenInfoFetching>,
}

impl<Factory> PoolInfoFetcher<Factory> {
    pub fn new(
        vault: BalancerV2Vault,
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
    fn base_pool_at(&self, pool_address: H160) -> BalancerV2BasePool {
        let web3 = self.vault.raw_instance().web3();
        BalancerV2BasePool::at(&web3, pool_address)
    }

    /// Retrieves the scaling exponents for the specified tokens.
    async fn scaling_exponents(&self, tokens: &[H160]) -> Result<Vec<u8>> {
        let token_infos = self.token_infos.get_token_infos(tokens).await;
        tokens
            .iter()
            .map(|token| {
                let decimals = token_infos
                    .get(token)
                    .ok_or_else(|| anyhow!("missing token info for {:?}", token))?
                    .decimals
                    .ok_or_else(|| anyhow!("missing decimals for token {:?}", token))?;
                scaling_exponent_from_decimals(decimals)
            })
            .collect()
    }

    async fn fetch_common_pool_info(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<PoolInfo> {
        let pool = self.base_pool_at(pool_address);

        let pool_id = H256(pool.methods().get_pool_id().call().await?.0);
        let (tokens, _, _) = self
            .vault
            .methods()
            .get_pool_tokens(Bytes(pool_id.0))
            .call()
            .await?;
        let scaling_exponents = self.scaling_exponents(&tokens).await?;

        Ok(PoolInfo {
            id: pool_id,
            address: pool_address,
            tokens,
            scaling_exponents,
            block_created,
        })
    }

    fn fetch_common_pool_state(
        &self,
        pool: &PoolInfo,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolState>> {
        let pool_contract = self.base_pool_at(pool.address);
        let paused = pool_contract
            .get_paused_state()
            .block(block)
            .batch_call(batch);
        let swap_fee = pool_contract
            .get_swap_fee_percentage()
            .block(block)
            .batch_call(batch);
        let balances = self
            .vault
            .get_pool_tokens(Bytes(pool.id.0))
            .block(block)
            .batch_call(batch);

        // Because of a `mockall` limitation, we **need** the future returned
        // here to be `'static`. This requires us to clone and move `pool` into
        // the async closure - otherwise it would only live for as long as
        // `pool`, i.e. `'_`.
        let pool = pool.clone();
        async move {
            let (paused, _, _) = paused.await?;
            let swap_fee = Bfp::from_wei(swap_fee.await?);

            let (token_addresses, balances, _) = balances.await?;
            ensure!(pool.tokens == token_addresses, "pool token mismatch");
            let tokens = itertools::izip!(&pool.tokens, balances, &pool.scaling_exponents)
                .map(|(&address, balance, &scaling_exponent)| {
                    (
                        address,
                        TokenState {
                            balance,
                            scaling_exponent,
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
        pool_address: H160,
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
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>> {
        let pool_id = pool_info.common().id;
        let (common_pool_state, common_pool_state_ok) =
            share_common_pool_state(self.fetch_common_pool_state(pool_info.common(), batch, block));
        let pool_state =
            self.factory
                .fetch_pool_state(pool_info, common_pool_state_ok.boxed(), batch, block);

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
    pub id: H256,
    pub address: H160,
    pub tokens: Vec<H160>,
    pub scaling_exponents: Vec<u8>,
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
            scaling_exponents: pool
                .tokens
                .iter()
                .map(|token| scaling_exponent_from_decimals(token.decimals))
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub paused: bool,
    pub swap_fee: Bfp,
    pub tokens: BTreeMap<H160, TokenState>,
}

impl Default for PoolState {
    fn default() -> Self {
        Self {
            paused: false,
            swap_fee: 0.into(),
            tokens: BTreeMap::new(),
        }
    }
}

/// Common pool token state information that is shared among all pool types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub balance: U256,
    pub scaling_exponent: u8,
}

/// Converts a token decimal count to its corresponding scaling exponent.
fn scaling_exponent_from_decimals(decimals: u8) -> Result<u8> {
    // Technically this should never fail for Balancer Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts)
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/deployments-latest/pkg/pool-utils/contracts/BasePool.sol#L476-L487
    18u8.checked_sub(decimals)
        .ok_or_else(|| anyhow!("unsupported token with more than 18 decimals"))
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
    let (pool_sender, pool_receiver) = oneshot::channel();

    let result = fut.inspect(|pool_result| {
        // We can't clone `anyhow::Error` so just clone the pool data and use
        // an empty `()` error.
        let pool_result = pool_result.as_ref().map(Clone::clone).map_err(|_| ());
        // Ignore error if the shared future was dropped.
        let _ = pool_sender.send(pool_result);
    });
    let shared = async move {
        pool_receiver
            .now_or_never()
            .expect("result future is still pending")
            .expect("result future was dropped")
            .expect("result future resolved to an error")
    };

    (result, shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sources::balancer_v2::{
            graph_api::{PoolType, Token},
            pools::{weighted, MockFactoryIndexing, PoolKind},
        },
        token_info::{MockTokenInfoFetching, TokenInfo},
    };
    use anyhow::bail;
    use contracts::BalancerV2WeightedPool;
    use ethcontract::U256;
    use ethcontract_mock::Mock;
    use futures::future;
    use maplit::{btreemap, hashmap};
    use mockall::predicate;

    #[tokio::test]
    async fn fetch_common_pool_info() {
        let pool_id = H256([0x90; 32]);
        let tokens = [H160([1; 20]), H160([2; 20]), H160([3; 20])];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2BasePool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2BasePool::signatures().get_pool_id())
            .returns(Bytes(pool_id.0));

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .predicate((predicate::eq(Bytes(pool_id.0)),))
            .returns((tokens.to_vec(), vec![], U256::zero()));

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
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = pool_info_fetcher
            .fetch_common_pool_info(pool.address(), 1337)
            .await
            .unwrap();

        assert_eq!(
            pool_info,
            PoolInfo {
                id: pool_id,
                address: pool.address(),
                tokens: tokens.to_vec(),
                scaling_exponents: vec![0, 0, 12],
                block_created: 1337,
            }
        );
    }

    #[tokio::test]
    async fn fetch_common_pool_state() {
        let pool_id = H256([0x90; 32]);
        let tokens = [H160([1; 20]), H160([2; 20]), H160([3; 20])];
        let balances = [bfp!("1000.0"), bfp!("10.0"), bfp!("15.0")];
        let scaling_exponents = [0, 0, 12];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2BasePool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2BasePool::signatures().get_paused_state())
            .returns((false, 0.into(), 0.into()));
        pool.expect_call(BalancerV2BasePool::signatures().get_swap_fee_percentage())
            .returns(bfp!("0.003").as_uint256());

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .predicate((predicate::eq(Bytes(pool_id.0)),))
            .returns((
                tokens.to_vec(),
                balances.into_iter().map(Bfp::as_uint256).collect(),
                0.into(),
            ));

        let token_infos = MockTokenInfoFetching::new();

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = PoolInfo {
            id: pool_id,
            address: pool.address(),
            tokens: tokens.to_vec(),
            scaling_exponents: scaling_exponents.to_vec(),
            block_created: 1337,
        };

        let pool_state = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let block = web3.eth().block_number().await.unwrap();

            let pool_state =
                pool_info_fetcher.fetch_common_pool_state(&pool_info, &mut batch, block.into());

            batch.execute_all(100).await;
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
                        scaling_exponent: scaling_exponents[0],
                    },
                    tokens[1] => TokenState {
                        balance: balances[1].as_uint256(),
                        scaling_exponent: scaling_exponents[1],
                    },
                    tokens[2] => TokenState {
                        balance: balances[2].as_uint256(),
                        scaling_exponent: scaling_exponents[2],
                    },
                },
            }
        );
    }

    #[tokio::test]
    async fn fetch_state_errors_on_token_mismatch() {
        let tokens = [H160([1; 20]), H160([2; 20]), H160([3; 20])];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2BasePool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2BasePool::signatures().get_paused_state())
            .returns((false, 0.into(), 0.into()));
        pool.expect_call(BalancerV2BasePool::signatures().get_swap_fee_percentage())
            .returns(0.into());

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .predicate((predicate::eq(Bytes(Default::default())),))
            .returns((
                vec![H160([1; 20]), H160([4; 20])],
                vec![0.into(), 0.into()],
                0.into(),
            ));

        let token_infos = MockTokenInfoFetching::new();

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        let pool_info = PoolInfo {
            id: Default::default(),
            address: pool.address(),
            tokens: tokens.to_vec(),
            scaling_exponents: vec![0, 0, 0],
            block_created: 1337,
        };

        let pool_state = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let block = web3.eth().block_number().await.unwrap();

            let pool_state =
                pool_info_fetcher.fetch_common_pool_state(&pool_info, &mut batch, block.into());

            batch.execute_all(100).await;
            pool_state.await
        };

        assert!(pool_state.is_err());
    }

    #[tokio::test]
    async fn fetch_specialized_pool_state() {
        let swap_fee = bfp!("0.003");

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2WeightedPool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2WeightedPool::signatures().get_paused_state())
            .returns((false, 0.into(), 0.into()));
        pool.expect_call(BalancerV2WeightedPool::signatures().get_swap_fee_percentage())
            .returns(swap_fee.as_uint256());

        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: H256([0x90; 32]),
                address: pool.address(),
                tokens: vec![H160([1; 20]), H160([2; 20]), H160([3; 20])],
                scaling_exponents: vec![0, 0, 12],
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
                        scaling_exponent: pool_info.common.scaling_exponents[0],
                    },
                    weight: pool_info.weights[0],
                },
                pool_info.common.tokens[1] => weighted::TokenState {
                    common: TokenState {
                        balance: bfp!("10.0").as_uint256(),
                        scaling_exponent: pool_info.common.scaling_exponents[1],
                    },
                    weight: pool_info.weights[1],
                },
                pool_info.common.tokens[2] => weighted::TokenState {
                    common: TokenState {
                        balance: bfp!("15.0").as_uint256(),
                        scaling_exponent: pool_info.common.scaling_exponents[2],
                    },
                    weight: pool_info.weights[2],
                },
            },
        };

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .predicate((predicate::eq(Bytes(pool_info.common.id.0)),))
            .returns((
                pool_info.common.tokens.clone(),
                pool_state
                    .tokens
                    .values()
                    .map(|token| token.common.balance)
                    .collect(),
                0.into(),
            ));

        let block = web3.eth().block_number().await.unwrap();

        let mut factory = MockFactoryIndexing::new();
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::eq(pool_info.clone()),
                predicate::always(),
                predicate::always(),
                predicate::eq(BlockId::from(block)),
            )
            .returning({
                let pool_state = pool_state.clone();
                move |_, _, _, _| future::ready(Ok(Some(pool_state.clone()))).boxed()
            });

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };

        let pool_status = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let pool_state = pool_info_fetcher.fetch_pool(&pool_info, &mut batch, block.into());

            batch.execute_all(100).await;
            pool_state.await.unwrap()
        };

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
        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2WeightedPool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2WeightedPool::signatures().get_paused_state())
            .returns((true, 0.into(), 0.into()));
        pool.expect_call(BalancerV2WeightedPool::signatures().get_swap_fee_percentage())
            .returns(Default::default());

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .returns(Default::default());

        let mut factory = MockFactoryIndexing::new();
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::always(),
                predicate::always(),
                predicate::always(),
                predicate::always(),
            )
            .returning(|_, _, _, _| {
                future::ready(Ok(Some(weighted::PoolState {
                    swap_fee: Bfp::zero(),
                    tokens: Default::default(),
                })))
                .boxed()
            });

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };
        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: Default::default(),
                address: pool.address(),
                tokens: Default::default(),
                scaling_exponents: Default::default(),
                block_created: Default::default(),
            },
            weights: Default::default(),
        };

        let pool_status = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let block = web3.eth().block_number().await.unwrap();

            let pool_state = pool_info_fetcher.fetch_pool(&pool_info, &mut batch, block.into());

            batch.execute_all(100).await;
            pool_state.await.unwrap()
        };

        assert_eq!(pool_status, PoolStatus::Paused);
    }

    #[tokio::test]
    async fn fetch_specialized_pool_state_for_disabled_pool() {
        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2WeightedPool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2WeightedPool::signatures().get_paused_state())
            .returns((false, 0.into(), 0.into()));
        pool.expect_call(BalancerV2WeightedPool::signatures().get_swap_fee_percentage())
            .returns(Default::default());

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .returns(Default::default());

        let mut factory = MockFactoryIndexing::new();
        factory
            .expect_fetch_pool_state()
            .with(
                predicate::always(),
                predicate::always(),
                predicate::always(),
                predicate::always(),
            )
            .returning(|_, _, _, _| future::ready(Ok(None)).boxed());

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
            factory,
            token_infos: Arc::new(MockTokenInfoFetching::new()),
        };
        let pool_info = weighted::PoolInfo {
            common: PoolInfo {
                id: Default::default(),
                address: pool.address(),
                tokens: Default::default(),
                scaling_exponents: Default::default(),
                block_created: Default::default(),
            },
            weights: Default::default(),
        };

        let pool_status = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let block = web3.eth().block_number().await.unwrap();

            let pool_state = pool_info_fetcher.fetch_pool(&pool_info, &mut batch, block.into());

            batch.execute_all(100).await;
            pool_state.await.unwrap()
        };

        assert_eq!(pool_status, PoolStatus::Disabled);
    }

    #[tokio::test]
    async fn scaling_exponent_error_on_missing_info() {
        let mut token_infos = MockTokenInfoFetching::new();
        token_infos
            .expect_get_token_infos()
            .returning(|_| hashmap! {});

        let pool_info_fetcher = PoolInfoFetcher {
            vault: dummy_contract!(BalancerV2Vault, H160([0xba; 20])),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        assert!(pool_info_fetcher
            .scaling_exponents(&[H160([0xff; 20])])
            .await
            .is_err());
    }

    #[tokio::test]
    async fn scaling_exponent_error_on_missing_decimals() {
        let token = H160([0xff; 20]);
        let mut token_infos = MockTokenInfoFetching::new();
        token_infos.expect_get_token_infos().returning(move |_| {
            hashmap! {
                token => TokenInfo { decimals: None, symbol: None },
            }
        });

        let pool_info_fetcher = PoolInfoFetcher {
            vault: dummy_contract!(BalancerV2Vault, H160([0xba; 20])),
            factory: MockFactoryIndexing::new(),
            token_infos: Arc::new(token_infos),
        };
        assert!(pool_info_fetcher.scaling_exponents(&[token]).await.is_err());
    }

    #[test]
    fn convert_graph_pool_to_common_pool_info() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: H256([4; 32]),
            address: H160([3; 20]),
            factory: H160([0xfb; 20]),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: H160([0x33; 20]),
                    decimals: 3,
                    weight: None,
                },
                Token {
                    address: H160([0x44; 20]),
                    decimals: 18,
                    weight: None,
                },
            ],
        };

        assert_eq!(
            PoolInfo::from_graph_data(&pool, 42).unwrap(),
            PoolInfo {
                id: H256([4; 32]),
                address: H160([3; 20]),
                tokens: vec![H160([0x33; 20]), H160([0x44; 20])],
                scaling_exponents: vec![15, 0],
                block_created: 42,
            }
        );
    }

    #[test]
    fn pool_conversion_insufficient_tokens() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0; 20]),
            swap_enabled: true,
            tokens: vec![Token {
                address: H160([2; 20]),
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
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0; 20]),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: H160([2; 20]),
                    decimals: 19,
                    weight: Some("1.337".parse().unwrap()),
                },
                Token {
                    address: H160([3; 20]),
                    decimals: 18,
                    weight: Some("1.337".parse().unwrap()),
                },
            ],
        };
        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[test]
    fn scaling_exponent_from_decimals_ok_and_err() {
        for i in 0..=18 {
            assert_eq!(scaling_exponent_from_decimals(i).unwrap(), 18u8 - i);
        }
        assert_eq!(
            scaling_exponent_from_decimals(19).unwrap_err().to_string(),
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
}
