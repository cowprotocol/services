#[derive(Clone, Debug)]
pub enum FeePolicy {
    /// Applies to limit orders only.
    /// This fee should be taken if the solver provided good enough solution
    /// that even after the surplus fee is taken, there is still more
    /// surplus left above whatever the user expects [order limit price
    /// vs best quote].
    /// The fee is taken in `sell` token for `buy` orders and in `buy`
    /// token for `sell` orders.
    QuoteDeviation {
        /// Percentage of the order's `available surplus` should be taken as a
        /// protocol fee.
        ///
        /// `Available surplus` is the difference between the executed_price
        /// (adjusted by surplus_fee) and the closer of the two: order
        /// limit_price or best_quote. For out-of-market limit orders,
        /// order limit price is closer to the executed price. For
        /// in-market limit orders, best quote is closer to the executed
        /// price.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        volume_cap_factor: f64,
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
