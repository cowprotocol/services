//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod zeroex;

use crate::price_estimation::Query;
use ethcontract::U256;
use thiserror::Error;

/// Find a trade for a token pair.
///
/// This is similar to the `PriceEstimating` interface, but it expects calldata
/// to also be produced.
#[mockall::automock]
#[async_trait::async_trait]
pub trait TradeFinding: Send + Sync + 'static {
    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError>;
}

/// A trade.
pub struct Trade {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub data: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum TradeError {
    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
