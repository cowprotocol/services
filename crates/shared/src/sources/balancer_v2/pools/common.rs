//! Module with data types and logic common to multiple Balancer pool types

use crate::sources::balancer_v2::graph_api::{PoolData, PoolType};
use anyhow::{anyhow, ensure, Result};
use contracts::{BalancerV2BasePool, BalancerV2Vault};
use ethcontract::{batch::CallBatch, Bytes, H160, H256, U256};

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
}

impl PoolInfoFetcher {
    pub fn new(vault: BalancerV2Vault) -> Self {
        Self { vault }
    }
}

#[async_trait::async_trait]
impl PoolInfoFetching for PoolInfoFetcher {
    /// Could result in ethcontract::{NodeError, MethodError or ContractError}
    async fn pool_info(&self, pool_address: H160, block_created: u64) -> Result<PoolInfo> {
        let web3 = self.vault.raw_instance().web3();
        let pool = BalancerV2BasePool::at(&web3, pool_address);

        // Fetch the pool ID and scaling factors in a single call.
        let (pool_id, scaling_factors) = {
            let mut batch = CallBatch::new(web3.transport());
            let pool_id = pool.methods().get_pool_id().batch_call(&mut batch);
            let scaling_factors = pool.methods().get_scaling_factors().batch_call(&mut batch);
            batch.execute_all(2).await;

            (H256(pool_id.await?.0), scaling_factors.await?)
        };

        let (tokens, _, _) = self
            .vault
            .methods()
            .get_pool_tokens(Bytes(pool_id.0))
            .call()
            .await?;
        let scaling_exponents = scaling_exponents_from_factors(&scaling_factors)?;

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

/// Converts a slice of scaling factors to their corresponding exponents.
fn scaling_exponents_from_factors(factors: &[U256]) -> Result<Vec<u8>> {
    // Technically this should never fail for Balancer Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts).

    let ten = U256::from(10);
    factors
        .iter()
        .copied()
        .map(|mut factor| {
            let mut exponent = 0u8;
            while factor > U256::one() {
                ensure!(exponent < 18, "scaling factor overflow");
                ensure!((factor % ten).is_zero(), "scaling factor not a power of 10");
                exponent += 1;
                factor /= ten;
            }

            Ok(exponent)
        })
        .collect()
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
    use crate::sources::balancer_v2::graph_api::{PoolType, Token};
    use ethcontract_mock::Mock;
    use mockall::predicate;

    #[tokio::test]
    async fn fetch_pool_info() {
        let pool_id = H256([0x90; 32]);
        let tokens = [H160([1; 20]), H160([2; 20]), H160([3; 20])];
        let scaling_factors = [U256::one(), U256::from(1), U256::from(1_000_000_000)];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2BasePool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2BasePool::signatures().get_pool_id())
            .returns(Bytes(pool_id.0));
        pool.expect_call(BalancerV2BasePool::signatures().get_scaling_factors())
            .returns(scaling_factors.to_vec());

        let vault = mock.deploy(BalancerV2Vault::raw_contract().abi.clone());
        vault
            .expect_call(BalancerV2Vault::signatures().get_pool_tokens())
            .predicate((predicate::eq(Bytes(pool_id.0)),))
            .returns((tokens.to_vec(), vec![], U256::zero()));

        let pool_info_fetcher = PoolInfoFetcher {
            vault: BalancerV2Vault::at(&web3, vault.address()),
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
                scaling_exponents: vec![0, 0, 9],
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
    fn scaling_exponents_from_factors_ok_and_err() {
        let scaling_exponents = scaling_exponents_from_factors(
            &(0..=18)
                .map(|i| U256::from(10_u128.pow(i)))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(scaling_exponents, (0..=18).collect::<Vec<_>>());

        assert!(scaling_exponents_from_factors(&[19.into()]).is_err());
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
