//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod external;
pub mod oneinch;
pub mod paraswap;
pub mod zeroex;

use {
    crate::price_estimation::{PriceEstimationError, Query},
    anyhow::Result,
    contracts::{dummy_contract, ERC20},
    ethcontract::{Bytes, H160, U256},
    model::interaction::InteractionData,
    serde::Serialize,
    thiserror::Error,
};

/// Returns the default time limit used for quoting with external co-located
/// solvers.
pub fn time_limit() -> chrono::Duration {
    chrono::Duration::seconds(5)
}

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
}

/// A trade.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub interactions: Vec<InteractionWithMeta>,
    pub solver: H160,
}

impl Trade {
    /// Creates a new `Trade` instance for a swap with a DEX with the specified
    /// required token approval target.
    pub fn swap(
        in_token: H160,
        out_amount: U256,
        gas_estimate: u64,
        approval_target: Option<H160>,
        swap: InteractionWithMeta,
        solver: H160,
    ) -> Self {
        let interactions = match approval_target {
            Some(spender) => {
                let token = dummy_contract!(ERC20, in_token);
                let approve = |amount| {
                    let tx = token.methods().approve(spender, amount).tx;
                    InteractionWithMeta {
                        interaction: Interaction {
                            target: tx.to.unwrap(),
                            value: tx.value.unwrap_or_default(),
                            data: tx.data.unwrap().0,
                        },
                        internalize: swap.internalize,
                        input_tokens: vec![],
                    }
                };

                // For approvals, reset the approval completely. Some tokens
                // require this such as Tether USD.
                vec![approve(U256::zero()), approve(U256::max_value()), swap]
            }
            None => vec![swap],
        };

        Self {
            out_amount,
            gas_estimate,
            interactions,
            solver,
        }
    }
}

/// Data for a raw interaction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
    pub data: Vec<u8>,
}

/// [`Interaction`] plus some metadata used to manage internalization of the
/// interaction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct InteractionWithMeta {
    pub interaction: Interaction,
    pub internalize: bool,
    pub input_tokens: Vec<H160>,
}

impl Interaction {
    pub fn encode(&self) -> EncodedInteraction {
        (self.target, self.value, Bytes(self.data.clone()))
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
    interactions
        .iter()
        .cloned()
        .map(|i| Interaction {
            target: i.target,
            value: i.value,
            data: i.call_data,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn trade_for_swap() {
        let swap_interaction = InteractionWithMeta {
            interaction: Interaction {
                target: H160([0xaa; 20]),
                value: 42.into(),
                data: vec![1, 2, 3, 4],
            },
            internalize: true,
            input_tokens: vec![H160([0xdd; 20])],
        };

        let trade = Trade::swap(
            H160([0xdd; 20]),
            1.into(),
            2,
            Some(H160([0xee; 20])),
            swap_interaction.clone(),
            H160([1; 20]),
        );

        assert_eq!(
            trade.interactions,
            [
                InteractionWithMeta {
                    interaction: Interaction {
                        target: H160([0xdd; 20]),
                        value: U256::zero(),
                        data: hex!(
                            "095ea7b3
                             000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                             0000000000000000000000000000000000000000000000000000000000000000"
                        )
                        .to_vec(),
                    },
                    internalize: true,
                    input_tokens: vec![],
                },
                InteractionWithMeta {
                    interaction: Interaction {
                        target: H160([0xdd; 20]),
                        value: U256::zero(),
                        data: hex!(
                            "095ea7b3
                             000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                        )
                        .to_vec()
                    },
                    internalize: true,
                    input_tokens: vec![],
                },
                swap_interaction,
            ]
        );
    }
}
