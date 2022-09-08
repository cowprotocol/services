//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod zeroex;

use crate::price_estimation::Query;
use ethcontract::{H160, U256};
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trade {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub approval_spender: Option<H160>,
    pub interaction: Interaction,
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
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
