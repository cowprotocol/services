//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `WeightedPools`
//! to be consumed by external users (e.g. Price Estimators and Solvers).
use anyhow::Result;
use model::TokenPair;
use std::collections::HashSet;

use crate::balancer::{
    event_handler::BalancerPoolRegistry,
    pool_storage::{RegisteredWeightedPool, WeightedPool},
};
use crate::pool_fetching::{handle_contract_error, MAX_BATCH_SIZE};
use crate::recent_block_cache::Block;
use crate::Web3;
use contracts::BalancerV2Vault;
use ethcontract::batch::CallBatch;
use ethcontract::errors::MethodError;
use ethcontract::{BlockId, Bytes, H160, U256};

#[async_trait::async_trait]
pub trait WeightedPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>>;
}

pub struct BalancerPoolFetcher {
    pool_registry: BalancerPoolRegistry,
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
            .pool_registry
            .get_pools_containing_token_pairs(token_pairs)
            .await
            .into_iter()
            .map(|registered_pool| {
                let reserves = self
                    .vault
                    .get_pool_tokens(Bytes(registered_pool.pool_id.0))
                    .block(block)
                    .batch_call(&mut batch);
                async move {
                    FetchedWeightedPool {
                        registered_pool,
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
    registered_pool: RegisteredWeightedPool,
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
            acc.push(WeightedPool::new(fetched_pool.registered_pool, balances));
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
            registered_pool: RegisteredWeightedPool::default(),
            reserves: Err(ethcontract_error::testing_node_error()),
        }];
        assert!(handle_results(results).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let results = vec![
            FetchedWeightedPool {
                registered_pool: RegisteredWeightedPool::default(),
                reserves: Err(ethcontract_error::testing_contract_error()),
            },
            FetchedWeightedPool {
                registered_pool: RegisteredWeightedPool::default(),
                reserves: Ok((vec![], vec![], U256::zero())),
            },
        ];
        assert_eq!(handle_results(results).unwrap().len(), 1);
    }
}
