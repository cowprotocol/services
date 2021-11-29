//! Module with data types and logic common to multiple Balancer pool types

use super::PoolIndexing;
use crate::sources::balancer_v2::graph_api::PoolData;
use anyhow::{anyhow, ensure, Result};
use ethcontract::{H160, H256};

/// Common pool data shared across all Balancer pools.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub id: H256,
    pub address: H160,
    pub tokens: Vec<H160>,
    pub scaling_exponents: Vec<u8>,
    pub block_created: u64,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
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

    fn common(&self) -> &PoolInfo {
        self
    }
}

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
