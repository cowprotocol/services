//! Module with data types and logic common to multiple Balancer pool types

use super::PoolIndexing;
use crate::sources::balancer_v2::graph_api::PoolData;
use anyhow::Result;
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
    fn from_graph_data(pool: PoolData, block_created: u64) -> Result<Self> {
        todo!()
    }

    fn common(&self) -> &PoolInfo {
        &self
    }
}

#[cfg(test)]
pub fn common_pool(seed: u8) -> PoolInfo {
    PoolInfo {
        id: H256([seed; 32]),
        address: H160([seed; 20]),
        tokens: vec![H160([seed; 20]), H160([seed + 1; 20])],
        scaling_exponents: vec![0, 0],
        block_created: seed as _,
    }
}
