//! A module for abstracting a component that can produce a quote with calldata
//! for a specified token pair and amount.

pub mod external;

use {
    crate::{
        conversions::U256Ext,
        price_estimation::{PriceEstimationError, Query},
        trade_finding::external::dto,
    },
    anyhow::{Context, Result},
    derive_more::Debug,
    ethcontract::{Bytes, H160, U256},
    model::{interaction::InteractionData, order::OrderKind},
    num::CheckedDiv,
    number::conversions::big_rational_to_u256,
    serde::Serialize,
    std::{collections::HashMap, ops::Mul},
    thiserror::Error,
};

/// Find a trade for a token pair.
///
/// This is similar to the `PriceEstimating` interface, but it expects calldata
/// to also be produced.
#[async_trait::async_trait]
pub trait TradeFinding: Send + Sync + 'static {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError>;
    async fn get_trade(&self, query: &Query) -> Result<TradeKind, TradeError>;
}

/// A quote.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Quote {
    pub out_amount: U256,
    pub gas_estimate: u64,
    pub solver: H160,
    pub interactions: Vec<InteractionData>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeKind {
    Legacy(LegacyTrade),
    Regular(Trade),
}

impl Default for TradeKind {
    fn default() -> Self {
        Self::Legacy(LegacyTrade::default())
    }
}

impl TradeKind {
    pub fn gas_estimate(&self) -> Option<u64> {
        match self {
            TradeKind::Legacy(trade) => trade.gas_estimate,
            TradeKind::Regular(trade) => trade.gas_estimate,
        }
    }

    pub fn solver(&self) -> H160 {
        match self {
            TradeKind::Legacy(trade) => trade.solver,
            TradeKind::Regular(trade) => trade.solver,
        }
    }

    pub fn tx_origin(&self) -> Option<H160> {
        match self {
            TradeKind::Legacy(trade) => trade.tx_origin,
            TradeKind::Regular(trade) => trade.tx_origin,
        }
    }

    pub fn out_amount(
        &self,
        buy_token: &H160,
        sell_token: &H160,
        in_amount: &U256,
        order_kind: &OrderKind,
    ) -> Result<U256> {
        match self {
            TradeKind::Legacy(trade) => Ok(trade.out_amount),
            TradeKind::Regular(trade) => {
                trade.out_amount(buy_token, sell_token, in_amount, order_kind)
            }
        }
    }

    pub fn interactions(&self) -> Vec<Interaction> {
        match self {
            TradeKind::Legacy(trade) => trade.interactions.clone(),
            TradeKind::Regular(trade) => trade.interactions.clone(),
        }
    }

    pub fn pre_interactions(&self) -> Vec<Interaction> {
        match self {
            TradeKind::Legacy(_) => Vec::new(),
            TradeKind::Regular(trade) => trade.pre_interactions.clone(),
        }
    }
}

/// A legacy trade.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LegacyTrade {
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

/// A trade with JIT orders.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub clearing_prices: HashMap<H160, U256>,
    /// How many units of gas this trade will roughly cost.
    pub gas_estimate: Option<u64>,
    /// The onchain calls to run before sending user funds to the settlement
    /// contract.
    pub pre_interactions: Vec<Interaction>,
    /// Interactions needed to produce the expected trade amount.
    pub interactions: Vec<Interaction>,
    /// Which solver provided this trade.
    pub solver: H160,
    /// If this is set the quote verification need to use this as the
    /// `tx.origin` to make the quote pass the simulation.
    pub tx_origin: Option<H160>,
    pub jit_orders: Vec<dto::JitOrder>,
}

impl Trade {
    pub fn out_amount(
        &self,
        buy_token: &H160,
        sell_token: &H160,
        in_amount: &U256,
        order_kind: &OrderKind,
    ) -> Result<U256> {
        let sell_price = self
            .clearing_prices
            .get(sell_token)
            .context("clearing sell price missing")?
            .to_big_rational();
        let buy_price = self
            .clearing_prices
            .get(buy_token)
            .context("clearing buy price missing")?
            .to_big_rational();
        let order_amount = in_amount.to_big_rational();

        let out_amount = match order_kind {
            OrderKind::Sell => order_amount
                .mul(&sell_price)
                .checked_div(&buy_price)
                .context("div by zero: buy price")?
                .ceil(), /* `ceil` is used to compute buy amount only: https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L389-L411 */
            OrderKind::Buy => order_amount
                .mul(&buy_price)
                .checked_div(&sell_price)
                .context("div by zero: sell price")?,
        };

        big_rational_to_u256(&out_amount).context("out amount is not a valid U256")
    }
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Debug)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
    #[debug("0x{}", hex::encode::<&[u8]>(data.as_ref()))]
    pub data: Vec<u8>,
}

impl Interaction {
    pub fn encode(&self) -> EncodedInteraction {
        (self.target, self.value, Bytes(self.data.clone()))
    }

    pub fn to_interaction_data(&self) -> InteractionData {
        InteractionData {
            target: self.target,
            value: self.value,
            call_data: self.data.clone(),
        }
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

pub fn map_interactions_data(interactions: &[Interaction]) -> Vec<InteractionData> {
    interactions
        .iter()
        .cloned()
        .map(|i| i.to_interaction_data())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_interaction() {
        let interaction = Interaction {
            target: H160::zero(),
            value: U256::one(),
            data: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        };

        let interaction_debug = format!("{:?}", interaction);

        assert_eq!(
            interaction_debug,
            "Interaction { target: 0x0000000000000000000000000000000000000000, value: 1, data: \
             0x010203040506 }"
        )
    }
}
