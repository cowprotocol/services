//! Module with data types and logic common to multiple Balancer pool types

use crate::{
    sources::balancer_v2::graph_api::{PoolData, PoolType},
    token_info::TokenInfoFetching,
};
use anyhow::{anyhow, ensure, Result};
use contracts::{BalancerV2BasePool, BalancerV2Vault};
use ethcontract::{Bytes, H160, H256};
use std::sync::Arc;

/// Trait for fetching common pool data by address.
#[mockall::automock]
#[async_trait::async_trait]
pub trait PoolInfoFetching: Send + Sync {
    async fn pool_info(&self, pool_address: H160, block_created: u64) -> Result<PoolInfo>;
}

/// Via `PoolInfoFetcher` leverages a combination of `Web3` and `TokenInfoFetching`
/// to recover common `PoolInfo` from an address.
pub struct PoolInfoFetcher {
    vault: BalancerV2Vault,
    token_infos: Arc<dyn TokenInfoFetching>,
}

impl PoolInfoFetcher {
    pub fn new(vault: BalancerV2Vault, token_infos: Arc<dyn TokenInfoFetching>) -> Self {
        Self { vault, token_infos }
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
}

#[async_trait::async_trait]
impl PoolInfoFetching for PoolInfoFetcher {
    /// Could result in ethcontract::{NodeError, MethodError or ContractError}
    async fn pool_info(&self, pool_address: H160, block_created: u64) -> Result<PoolInfo> {
        let web3 = self.vault.raw_instance().web3();
        let pool = BalancerV2BasePool::at(&web3, pool_address);

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

/// Converts a token decimal count to its corresponding scaling exponent.
fn scaling_exponent_from_decimals(decimals: u8) -> Result<u8> {
    // Technically this should never fail for Balancer Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts)
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/deployments-latest/pkg/pool-utils/contracts/BasePool.sol#L476-L487
    18u8.checked_sub(decimals)
        .ok_or_else(|| anyhow!("unsupported token with more than 18 decimals"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sources::balancer_v2::graph_api::{PoolType, Token},
        token_info::{MockTokenInfoFetching, TokenInfo},
    };
    use ethcontract::U256;
    use ethcontract_mock::Mock;
    use maplit::hashmap;
    use mockall::predicate;

    #[tokio::test]
    async fn fetch_pool_info() {
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
            token_infos: Arc::new(token_infos),
        };
        let pool_info = pool_info_fetcher
            .pool_info(pool.address(), 1337)
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
        )
    }

    #[test]
    fn convert_graph_pool_to_common_pool_info() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: H256([4; 32]),
            address: H160([3; 20]),
            factory: H160([0xfb; 20]),
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
}
