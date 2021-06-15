use anyhow::Result;
use model::TokenPair;
use std::collections::{HashMap, HashSet};

use crate::balancer::event_handler::{PoolRegistry, RegisteredWeightedPool};
use crate::pool_fetching::{handle_contract_error, MAX_BATCH_SIZE};
use crate::recent_block_cache::Block;
use crate::Web3;
use contracts::BalancerV2Vault;
use ethcontract::batch::CallBatch;
use ethcontract::errors::MethodError;
use ethcontract::{BlockId, Bytes, H160, H256, U256};

pub struct PoolTokenState {
    pub balance: U256,
    pub weight: U256,
    pub scaling_exponent: u8,
}

pub struct WeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub reserves: HashMap<H160, PoolTokenState>,
}

impl WeightedPool {
    fn new(pool_data: RegisteredWeightedPool, balances: Vec<U256>) -> Self {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (i, balance) in balances.into_iter().enumerate() {
            reserves.insert(
                pool_data.tokens[i],
                PoolTokenState {
                    balance,
                    weight: pool_data.normalized_weights[i],
                    scaling_exponent: pool_data.scaling_exponents[i],
                },
            );
        }
        WeightedPool {
            pool_id: pool_data.pool_id,
            pool_address: pool_data.pool_address,
            reserves,
        }
    }
}

#[async_trait::async_trait]
pub trait WeightedPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>>;
}

pub struct BalancerPoolFetcher {
    pool_data: PoolRegistry,
    vault: BalancerV2Vault,
    web3: Web3,
}

#[async_trait::async_trait]
impl WeightedPoolFetching for BalancerPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>> {
        let mut batch = CallBatch::new(self.web3.transport());
        let block = BlockId::Number(at_block.into());
        let futures = self
            .pool_data
            .pools_containing_token_pairs(token_pairs)
            .into_iter()
            .map(|weighted_pool| {
                let reserves = self
                    .vault
                    .get_pool_tokens(Bytes(weighted_pool.pool_id.0))
                    .block(block)
                    .batch_call(&mut batch);
                async move {
                    FetchedWeightedPool {
                        pool_data: weighted_pool,
                        reserves: reserves.await,
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let mut results = Vec::new();
        for future in futures {
            // Batch has already been executed, so these awaits resolve immediately.
            results.push(future.await);
        }
        handle_results(results)
    }
}

/// An internal temporary struct used during pool fetching to handle errors.
struct FetchedWeightedPool {
    pool_data: RegisteredWeightedPool,
    /// getPoolTokens returns (Tokens, Balances, LastBlockUpdated)
    reserves: Result<(Vec<H160>, Vec<U256>, U256), MethodError>,
}

fn handle_results(results: Vec<FetchedWeightedPool>) -> Result<Vec<WeightedPool>> {
    results
        .into_iter()
        .try_fold(Vec::new(), |mut acc, fetched_pool| {
            let balances = match handle_contract_error(fetched_pool.reserves)? {
                // We only keep the balances entry of reserves query.
                Some(reserves) => reserves.1,
                None => return Ok(acc),
            };
            acc.push(WeightedPool::new(fetched_pool.pool_data, balances));
            Ok(acc)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethcontract_error;

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let results = vec![FetchedWeightedPool {
            pool_data: RegisteredWeightedPool::default(),
            reserves: Err(ethcontract_error::testing_node_error()),
        }];
        assert!(handle_results(results).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let results = vec![
            FetchedWeightedPool {
                pool_data: RegisteredWeightedPool::default(),
                reserves: Err(ethcontract_error::testing_contract_error()),
            },
            FetchedWeightedPool {
                pool_data: RegisteredWeightedPool::default(),
                reserves: Ok((vec![], vec![], U256::zero())),
            },
        ];
        assert_eq!(handle_results(results).unwrap().len(), 1);
    }
}
