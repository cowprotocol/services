//! In-memory models for Uniswap V3 pool state and ticks, produced by a
//! [`V3PoolDataSource`](super::V3PoolDataSource) and consumed by the pool
//! fetcher.

use {
    alloy::primitives::{Address, U256},
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
};

/// The pools a source knows about, anchored at `fetched_block_number`.
#[derive(Debug, Default, PartialEq)]
pub struct RegisteredPools {
    /// The block the data was fetched at.
    pub fetched_block_number: u64,
    /// The registered pools.
    pub pools: Vec<PoolData>,
}

/// Pool state plus active ticks for a set of pools. `fetched_block_number` is
/// the actual snapshot block, which may be later than the caller's requested
/// block.
#[derive(Debug, Default, PartialEq)]
pub struct PoolsWithTicks {
    pub fetched_block_number: u64,
    pub pools: Vec<PoolData>,
}

/// A Uniswap V3 pool's state.
///
/// `block_number` is the block at which the authoritative state (`liquidity` /
/// `sqrt_price` / `tick`) was sampled; the source stamps it per pool so drivers
/// can anchor per-pool event replay rather than a single global block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PoolData {
    pub id: Address,
    pub token0: Token,
    pub token1: Token,
    pub fee_tier: U256,
    pub liquidity: U256,
    pub sqrt_price: U256,
    pub tick: i32,
    pub ticks: Option<Vec<TickData>>,
    pub block_number: u64,
}

/// One active tick of a Uniswap V3 pool.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TickData {
    pub tick_idx: i32,
    pub liquidity_net: i128,
    pub pool_address: Address,
}

/// Serialized as part of [`PoolInfo`](super::pool_fetching::PoolInfo) for the
/// baseline solver, which expects `decimals` as a decimal string.
#[serde_as]
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize)]
pub struct Token {
    pub id: Address,
    #[serde_as(as = "DisplayFromStr")]
    pub decimals: u8,
}
