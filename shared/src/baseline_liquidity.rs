//! Module for loading all supported baseline liquidity sources.
//!
//! These liquidity sources are used by the local solving infrastructure,
//! including the baseline solver and optimization solver, as well as price
//! and fee estimation.

pub mod balancer;
pub mod uniswap;

use self::{balancer::BalancerV2Pool, uniswap::UniswapV2Pair};
use crate::baseline_solver::BaselineSolvable;
use anyhow::Result;
use ethcontract::{H160, U256};
use futures::future;
use model::TokenPair;
use num::BigRational;
use std::{collections::HashSet, sync::Arc};

/// A liquidity source
pub enum BaselineSource {
    UniswapV2,
    Sushi,
    Honeyswap,
    BalancerV2,
}

impl BaselineSource {
    /// Creates the default fetcher for the specifed source.
    ///
    /// Returns `None` if the source is not supported for the specifed network.
    pub fn fetcher(&self, chain_id: u64) -> Option<Arc<dyn LiquidityFetching>> {
        match (self, chain_id) {
            (BaselineSource::UniswapV2, 100) => None,
            (BaselineSource::Honeyswap, 1 | 4) => None,
            (BaselineSource::BalancerV2, 100) => None,
            _ => todo!(),
        }
    }
}

/// All supported baseline liquidity considered by the protocol.
pub enum BaselineLiquidity {
    UniswapV2(UniswapV2Pair),
    Sushi(UniswapV2Pair),
    Honeyswap(UniswapV2Pair),
    BalancerV2(BalancerV2Pool),
}

impl BaselineSolvable for BaselineLiquidity {
    // forward implementation to inner types.
    fn get_amount_out(&self, _: H160, _: U256, _: H160) -> Option<U256> {
        todo!()
    }
    fn get_amount_in(&self, _: H160, _: U256, _: H160) -> Option<U256> {
        todo!()
    }
    fn get_spot_price(&self, _: H160, _: H160) -> Option<BigRational> {
        todo!()
    }
    fn gas_cost(&self) -> usize {
        todo!()
    }
}

/// A trait used for fetching Baseline liquidity.
#[async_trait::async_trait]
pub trait LiquidityFetching: Send + Sync {
    async fn fetch(&self, pairs: HashSet<TokenPair>) -> Result<Vec<BaselineLiquidity>>;
}

pub struct BaselineLiquidityFetcher {
    sources: Vec<Arc<dyn LiquidityFetching>>,
}

impl BaselineLiquidityFetcher {
    /// Creates a new baseline liquidity fetcher using the specified sources.
    pub fn new(sources: &[BaselineSource], chain_id: u64) -> Self {
        Self {
            sources: sources
                .iter()
                .flat_map(|source| source.fetcher(chain_id))
                .collect(),
        }
    }
}

#[async_trait::async_trait]
impl LiquidityFetching for BaselineLiquidityFetcher {
    async fn fetch(&self, pairs: HashSet<TokenPair>) -> Result<Vec<BaselineLiquidity>> {
        Ok(future::try_join_all(
            self.sources
                .iter()
                .map(|source| source.fetch(pairs.clone())),
        )
        .await?
        .into_iter()
        .flatten()
        .collect())
    }
}
