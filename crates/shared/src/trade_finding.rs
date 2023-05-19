//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod external;
pub mod oneinch;
pub mod paraswap;
pub mod zeroex;

use {
    crate::price_estimation::{PriceEstimationError, Query},
    anyhow::Result,
    contracts::ERC20,
    ethcontract::{contract::MethodBuilder, tokens::Tokenize, web3::Transport, Bytes, H160, U256},
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
}

/// A trade.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub interactions: Vec<Interaction>,
}

impl Trade {
    /// Creates a new `Trade` instance for a swap with a DEX with the specified
    /// required token approval target.
    pub fn swap(
        in_token: H160,
        out_amount: U256,
        gas_estimate: u64,
        approval_target: Option<H160>,
        swap: Interaction,
    ) -> Self {
        let interactions = match approval_target {
            Some(spender) => {
                let token = dummy_contract!(ERC20, in_token);
                let approve =
                    |amount| Interaction::from_call(token.methods().approve(spender, amount));

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
        }
    }

    /// Converts a trade into a set of interactions for settlements.
    pub fn encode(&self) -> Vec<EncodedInteraction> {
        self.interactions.iter().map(|i| i.encode()).collect()
    }
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
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

pub type EncodedInteraction = (H160, U256, Bytes<Vec<u8>>);

#[derive(Debug, Error)]
pub enum TradeError {
    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType,

    #[error("Rate limited")]
    RateLimited,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<PriceEstimationError> for TradeError {
    fn from(err: PriceEstimationError) -> Self {
        match err {
            PriceEstimationError::NoLiquidity => Self::NoLiquidity,
            PriceEstimationError::UnsupportedOrderType => Self::UnsupportedOrderType,
            PriceEstimationError::RateLimited => Self::RateLimited,
            PriceEstimationError::Other(err) => Self::Other(err),
            _ => Self::Other(anyhow::anyhow!(err.to_string())),
        }
    }
}

impl Clone for TradeError {
    fn clone(&self) -> Self {
        match self {
            Self::NoLiquidity => Self::NoLiquidity,
            Self::UnsupportedOrderType => Self::UnsupportedOrderType,
            Self::RateLimited => Self::RateLimited,
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn trade_for_swap() {
        let trade = Trade::swap(
            H160([0xdd; 20]),
            1.into(),
            2,
            Some(H160([0xee; 20])),
            Interaction {
                target: H160([0xaa; 20]),
                value: 42.into(),
                data: vec![1, 2, 3, 4],
            },
        );

        assert_eq!(
            trade.interactions,
            [
                Interaction {
                    target: H160([0xdd; 20]),
                    value: U256::zero(),
                    data: hex!(
                        "095ea7b3
                         000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                         0000000000000000000000000000000000000000000000000000000000000000"
                    )
                    .to_vec(),
                },
                Interaction {
                    target: H160([0xdd; 20]),
                    value: U256::zero(),
                    data: hex!(
                        "095ea7b3
                         000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                         ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    )
                    .to_vec()
                },
                Interaction {
                    target: H160([0xaa; 20]),
                    value: 42.into(),
                    data: vec![1, 2, 3, 4],
                },
            ]
        );
    }

    #[test]
    fn encode_trade_to_interactions() {
        let trade = Trade {
            out_amount: Default::default(),
            gas_estimate: 0,
            interactions: vec![
                Interaction {
                    target: H160([0xaa; 20]),
                    value: 42.into(),
                    data: vec![1, 2, 3, 4],
                },
                Interaction {
                    target: H160([0xbb; 20]),
                    value: 43.into(),
                    data: vec![5, 6, 7, 8],
                },
            ],
        };

        assert_eq!(
            trade.encode(),
            vec![
                (H160([0xaa; 20]), U256::from(42), Bytes(vec![1, 2, 3, 4])),
                (H160([0xbb; 20]), U256::from(43), Bytes(vec![5, 6, 7, 8])),
            ],
        );
    }
}
