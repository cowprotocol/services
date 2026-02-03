//! Module implementing weighted pool specific indexing logic.

use {
    super::{FactoryIndexing, PoolIndexing, common},
    crate::sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    alloy::{eips::BlockId, primitives::Address},
    anyhow::{Result, anyhow},
    contracts::alloy::{
        BalancerV2WeightedPool,
        BalancerV2WeightedPoolFactory,
        BalancerV2WeightedPoolFactoryV3,
    },
    futures::{FutureExt as _, future::BoxFuture},
    std::collections::BTreeMap,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<Address, TokenState>,
    pub swap_fee: Bfp,
    pub version: Version,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub common: common::TokenState,
    pub weight: Bfp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Version {
    #[default]
    V0,
    V3Plus,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Weighted, pool, block_created)?,
            weights: pool
                .tokens
                .iter()
                .map(|token| {
                    token
                        .weight
                        .ok_or_else(|| anyhow!("missing weights for pool {:?}", pool.id))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPoolFactory::Instance {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        let pool_contract =
            BalancerV2WeightedPool::Instance::new(pool.address, self.provider().clone());
        let weights = pool_contract
            .getNormalizedWeights()
            .call()
            .await?
            .into_iter()
            .map(Bfp::from_wei)
            .collect();

        Ok(PoolInfo {
            common: pool,
            weights,
        })
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        _: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        pool_state(Version::V0, pool_info.clone(), common_pool_state)
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPoolFactoryV3::Instance {
    type PoolInfo = <BalancerV2WeightedPoolFactory::Instance as FactoryIndexing>::PoolInfo;
    type PoolState = <BalancerV2WeightedPoolFactory::Instance as FactoryIndexing>::PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        let v0 =
            BalancerV2WeightedPoolFactory::Instance::new(*self.address(), self.provider().clone());
        v0.specialize_pool_info(pool).await
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        _: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        pool_state(Version::V3Plus, pool_info.clone(), common_pool_state)
    }
}

fn pool_state(
    version: Version,
    info: PoolInfo,
    common: BoxFuture<'static, common::PoolState>,
) -> BoxFuture<'static, Result<Option<PoolState>>> {
    async move {
        let common = common.await;
        let tokens = common
            .tokens
            .into_iter()
            .zip(&info.weights)
            .map(|((address, common), &weight)| (address, TokenState { common, weight }))
            .collect();
        let swap_fee = common.swap_fee;

        Ok(Some(PoolState {
            tokens,
            swap_fee,
            version,
        }))
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sources::balancer_v2::graph_api::Token,
        alloy::{
            primitives::{Address, B256, U256},
            providers::{Provider, ProviderBuilder, mock::Asserter},
            sol_types::SolCall,
        },
        ethrpc::Web3,
        futures::future,
        maplit::btreemap,
    };

    #[test]
    fn convert_graph_pool_to_weighted_pool_info() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: B256::repeat_byte(2),
            address: Address::repeat_byte(1),
            factory: Address::repeat_byte(0xfa),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: Address::repeat_byte(0x11),
                    decimals: 1,
                    weight: Some(bfp!("1.337")),
                },
                Token {
                    address: Address::repeat_byte(0x22),
                    decimals: 2,
                    weight: Some(bfp!("4.2")),
                },
            ],
        };

        assert_eq!(
            PoolInfo::from_graph_data(&pool, 42).unwrap(),
            PoolInfo {
                common: common::PoolInfo {
                    id: B256::repeat_byte(2),
                    address: Address::repeat_byte(1),
                    tokens: vec![Address::repeat_byte(0x11), Address::repeat_byte(0x22)],
                    scaling_factors: vec![Bfp::exp10(17), Bfp::exp10(16)],
                    block_created: 42,
                },
                weights: vec![
                    Bfp::from_wei(U256::from(1_337_000_000_000_000_000_u128)),
                    Bfp::from_wei(U256::from(4_200_000_000_000_000_000_u128)),
                ],
            },
        );
    }

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: B256::repeat_byte(2),
            address: Address::repeat_byte(1),
            factory: Address::repeat_byte(0xfa),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: Address::repeat_byte(0x11),
                    decimals: 1,
                    weight: Some(bfp!("1.337")),
                },
                Token {
                    address: Address::repeat_byte(0x22),
                    decimals: 2,
                    weight: Some(bfp!("4.2")),
                },
            ],
        };

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[tokio::test]
    async fn fetch_weighted_pool() {
        let weights = [bfp!("0.5"), bfp!("0.25"), bfp!("0.25")];

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();

        let pool =
            BalancerV2WeightedPool::Instance::new(Address::new([0x90; 20]), provider.clone());
        let factory = BalancerV2WeightedPoolFactory::Instance::new(
            Address::new([0xfa; 20]),
            provider.clone(),
        );
        let get_normalized_weights_response =
            BalancerV2WeightedPool::BalancerV2WeightedPool::getNormalizedWeightsCall::abi_encode_returns(
                &weights.iter()
                    .map(|w| w.as_uint256())
                    .collect()
            );
        asserter.push_success(&get_normalized_weights_response);

        let pool = factory
            .specialize_pool_info(common::PoolInfo {
                id: B256::repeat_byte(0x90),
                tokens: vec![
                    Address::repeat_byte(1),
                    Address::repeat_byte(2),
                    Address::repeat_byte(3),
                ],
                address: *pool.address(),
                scaling_factors: vec![Bfp::exp10(0), Bfp::exp10(0), Bfp::exp10(0)],
                block_created: 42,
            })
            .await
            .unwrap();

        assert_eq!(pool.weights, weights);
    }

    #[tokio::test]
    async fn fetch_pool_state() {
        let tokens = btreemap! {
            Address::repeat_byte(1) => common::TokenState {
                balance: bfp!("1000.0").as_uint256(),
                scaling_factor: Bfp::exp10(0),
            },
            Address::repeat_byte(2) => common::TokenState {
                balance: U256::from(10_000_000),
                scaling_factor: Bfp::exp10(12),
            },
        };
        let weights = [bfp!("0.8"), bfp!("0.2")];
        let swap_fee = bfp!("0.003");

        let asserter = Asserter::new();
        asserter.push_success(&10);
        let web3 = Web3::with_asserter(asserter);

        let factory =
            BalancerV2WeightedPoolFactory::Instance::new(Address::default(), web3.alloy.clone());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: B256::repeat_byte(0x90),
                address: Address::repeat_byte(0x90),
                tokens: tokens.keys().copied().collect(),
                scaling_factors: tokens.values().map(|token| token.scaling_factor).collect(),
                block_created: 1337,
            },
            weights: weights.to_vec(),
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee,
            tokens,
        };

        let pool_state = {
            let block = web3.alloy.get_block_number().await.unwrap();

            let pool_state = factory.fetch_pool_state(
                &pool_info,
                future::ready(common_pool_state.clone()).boxed(),
                block.into(),
            );

            pool_state.await.unwrap()
        };

        let weighted_tokens = common_pool_state
            .tokens
            .into_iter()
            .zip(weights)
            .map(|((address, common), weight)| (address, TokenState { common, weight }))
            .collect();
        assert_eq!(
            pool_state,
            Some(PoolState {
                tokens: weighted_tokens,
                swap_fee,
                version: Version::V0,
            })
        );
    }
}
