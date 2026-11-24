//! The fee policies an order of an auction is charged.

use {
    crate::{boundary, domain},
    alloy::primitives::{Address, U256},
    configs::fee_factor::FeeFactor,
};

/// In case there is no quote, we assume 0 buy amount so that the order
/// ends up being considered out of market price.
pub(crate) fn reference_quote(
    order: &boundary::Order,
    quote: Option<&domain::Quote>,
) -> domain::Quote {
    quote.cloned().unwrap_or(domain::Quote {
        order_uid: order.metadata.uid.into(),
        sell_amount: order.data.sell_amount.into(),
        buy_amount: U256::ZERO.into(),
        fee: order.data.fee_amount.into(),
        solver: Address::ZERO,
    })
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Policy {
    /// If the order receives more than limit price, take the protocol fee as a
    /// percentage of the difference. The fee is taken in `sell` token for
    /// `buy` orders and in `buy` token for `sell` orders.
    Surplus {
        /// Factor of surplus the protocol charges as a fee.
        /// Surplus is the difference between executed price and limit price
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having a limit price
        /// of 1990USDC, their surplus is 10USDC. A factor of 0.5
        /// requires the solver to pay 5USDC to the protocol for
        /// settling this order.
        factor: FeeFactor,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: FeeFactor,
    },
    /// A price improvement corresponds to a situation where the order is
    /// executed at a better price than the top quote. The protocol fee in such
    /// case is calculated from a cut of this price improvement.
    PriceImprovement {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
        quote: Quote,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: FeeFactor,
    },
}

/// A fee policy together with the party it is charged for.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PolicyWithOrigin {
    pub policy: Policy,
    pub origin: Origin,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Configured by the protocol.
    Protocol,
    /// Requested by a partner via the order's app data.
    Partner,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Quote {
    /// The amount of the sell token.
    pub sell_amount: U256,
    /// The amount of the buy token.
    pub buy_amount: U256,
    /// The amount that needs to be paid, denominated in the sell token.
    pub fee: U256,
    pub solver: Address,
}

impl Quote {
    pub(crate) fn from_domain(value: &domain::Quote) -> Self {
        Self {
            sell_amount: value.sell_amount.0,
            buy_amount: value.buy_amount.0,
            fee: value.fee.0,
            solver: value.solver,
        }
    }
}
