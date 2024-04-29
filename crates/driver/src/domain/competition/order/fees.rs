use crate::domain::eth;

#[derive(Clone, Debug)]
pub enum FeePolicy {
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
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
    },
    /// A price improvement corresponds to a situation where the order is
    /// executed at a better price than the top quote. The protocol fee in such
    /// case is calculated as a percentage of this price improvement.
    PriceImprovement {
        /// Price improvement is the difference between executed price and the
        /// best quote or limit price, whichever is better for the user.
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having a best quote
        /// of 1995USDC and limit price of 1990USDC, their surplus is 10USDC
        /// while the price improvement is 5USDC. A factor of 0.1 requires the
        /// solver to pay 0.5USDC to the protocol for settling this order. In
        /// case the best quote was 1990USDC while the limit price was 1995USDC,
        /// the solver should also pay 0.5USDC to the protocol.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
        /// The best quote received.
        quote: Quote,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: f64,
    },
}

#[derive(Clone, Debug)]
pub struct Quote {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: eth::Asset,
}
