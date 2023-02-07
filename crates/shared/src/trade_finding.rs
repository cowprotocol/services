//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod external;
pub mod oneinch;
pub mod paraswap;
pub mod zeroex;

use {
    crate::{price_estimation::Query, rate_limiter::RateLimiterError},
    contracts::ERC20,
    ethcontract::{contract::MethodBuilder, tokens::Tokenize, web3::Transport, Bytes, H160, U256},
    serde::Deserialize,
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Quote {
    pub out_amount: U256,
    pub gas_estimate: u64,
}

/// A trade.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Trade {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub approval: Option<(H160, H160)>,
    pub interaction: Interaction,
}

impl Trade {
    /// Converts a trade into a set of interactions for settlements.
    pub fn encode(&self) -> SettlementInteractions {
        let pre_interactions = match self.approval {
            Some((token, spender)) => {
                let token = dummy_contract!(ERC20, token);
                let approve = |amount| {
                    Interaction::from_call(token.methods().approve(spender, amount)).encode()
                };

                // For approvals, reset the approval completely. Some tokens
                // require this such as Tether USD.
                vec![approve(U256::zero()), approve(U256::max_value())]
            }
            None => vec![],
        };
        let intra_interactions = vec![self.interaction.encode()];

        [pre_interactions, intra_interactions, vec![]]
    }
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
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

pub type SettlementInteractions = [Vec<EncodedInteraction>; 3];
pub type EncodedInteraction = (H160, U256, Bytes<Vec<u8>>);

pub fn convert_interactions(interactions: &[Vec<Interaction>; 3]) -> SettlementInteractions {
    [
        convert_interaction_group(&interactions[0]),
        convert_interaction_group(&interactions[1]),
        convert_interaction_group(&interactions[2]),
    ]
}

pub fn convert_interaction_group(interactions: &[Interaction]) -> Vec<EncodedInteraction> {
    interactions.iter().map(Interaction::encode).collect()
}

#[derive(Debug, Error)]
pub enum TradeError {
    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType,

    #[error(transparent)]
    RateLimited(#[from] RateLimiterError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Clone for TradeError {
    fn clone(&self) -> Self {
        match self {
            Self::NoLiquidity => Self::NoLiquidity,
            Self::UnsupportedOrderType => Self::UnsupportedOrderType,
            Self::RateLimited(err) => Self::RateLimited(err.clone()),
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn encode_trade_to_interactions() {
        let trade = Trade {
            out_amount: Default::default(),
            gas_estimate: 0,
            approval: Some((H160([0xdd; 20]), H160([0xee; 20]))),
            interaction: Interaction {
                target: H160([0xaa; 20]),
                value: 42.into(),
                data: vec![1, 2, 3, 4],
            },
        };

        assert_eq!(
            trade.encode(),
            [
                vec![
                    (
                        H160([0xdd; 20]),
                        U256::zero(),
                        Bytes(
                            hex!(
                                "095ea7b3
                             000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                             0000000000000000000000000000000000000000000000000000000000000000"
                            )
                            .to_vec()
                        ),
                    ),
                    (
                        H160([0xdd; 20]),
                        U256::zero(),
                        Bytes(
                            hex!(
                                "095ea7b3
                             000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
                             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                            )
                            .to_vec()
                        ),
                    )
                ],
                vec![(H160([0xaa; 20]), U256::from(42), Bytes(vec![1, 2, 3, 4]))],
                vec![],
            ]
        );
    }
}
