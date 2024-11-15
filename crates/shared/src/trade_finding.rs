//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod external;

use {
    crate::price_estimation::{PriceEstimationError, Query},
    anyhow::Result,
    derivative::Derivative,
    ethcontract::{contract::MethodBuilder, tokens::Tokenize, web3::Transport, Bytes, H160, U256},
    model::interaction::InteractionData,
    serde::Serialize,
    thiserror::Error,
};

/// Find a trade for a token pair.
///
/// This is similar to the `PriceEstimating` interface, but it expects calldata
/// to also be produced.
#[mockall::automock]
#[async_trait::async_trait]
pub trait TradeFinding: Send + Sync + 'static {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError>;
    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError>;
}

/// A quote.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Quote {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub solver: H160,
    pub call_data: Option<Vec<u8>>,
}

/// A trade.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    /// For sell orders: how many buy_tokens this trade will produce.
    /// For buy orders: how many sell_tokens this trade will cost.
    pub out_amount: U256,
    /// How many units of gas this trade will roughly cost.
    pub gas_estimate: Option<u64>,
    /// Interactions needed to produce the expected `out_amount`.
    pub interactions: Vec<Interaction>,
    /// Which solver provided this trade.
    pub solver: H160,
    /// If this is set the quote verification need to use this as the
    /// `tx.origin` to make the quote pass the simulation.
    pub tx_origin: Option<H160>,
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Derivative)]
#[derivative(Debug)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
    #[derivative(Debug(format_with = "crate::debug_bytes"))]
    pub data: Vec<u8>,
}

impl Interaction {
    pub fn from_call<T, R>(method: MethodBuilder<T, R>) -> Interaction
    where
        T: Transport,
        R: Tokenize,
    {
        Interaction {
            target: method.tx.to.unwrap(),
            value: method.tx.value.unwrap_or_default(),
            data: method.tx.data.unwrap().0,
        }
    }

    pub fn encode(&self) -> EncodedInteraction {
        (self.target, self.value, Bytes(self.data.clone()))
    }
}

impl From<InteractionData> for Interaction {
    fn from(interaction: InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            data: interaction.call_data,
        }
    }
}

pub type EncodedInteraction = (H160, U256, Bytes<Vec<u8>>);

#[derive(Debug, Error)]
pub enum TradeError {
    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type {0}")]
    UnsupportedOrderType(String),

    #[error("Deadline exceeded")]
    DeadlineExceeded,

    #[error("Rate limited")]
    RateLimited,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<PriceEstimationError> for TradeError {
    fn from(err: PriceEstimationError) -> Self {
        match err {
            PriceEstimationError::NoLiquidity => Self::NoLiquidity,
            PriceEstimationError::UnsupportedOrderType(order_type) => {
                Self::UnsupportedOrderType(order_type)
            }
            PriceEstimationError::UnsupportedToken { token, .. } => {
                Self::UnsupportedOrderType(format!("{token:#x}"))
            }
            PriceEstimationError::RateLimited => Self::RateLimited,
            PriceEstimationError::EstimatorInternal(err)
            | PriceEstimationError::ProtocolInternal(err) => Self::Other(err),
        }
    }
}

impl Clone for TradeError {
    fn clone(&self) -> Self {
        match self {
            Self::NoLiquidity => Self::NoLiquidity,
            Self::UnsupportedOrderType(order_type) => {
                Self::UnsupportedOrderType(order_type.clone())
            }
            Self::DeadlineExceeded => Self::DeadlineExceeded,
            Self::RateLimited => Self::RateLimited,
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

pub fn map_interactions(interactions: &[InteractionData]) -> Vec<Interaction> {
    interactions.iter().cloned().map(Into::into).collect()
}
