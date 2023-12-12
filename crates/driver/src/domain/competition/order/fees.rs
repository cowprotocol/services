#[derive(Clone, Debug)]
pub enum FeePolicy {
    /// If the order receives more than expected (positive deviation from quoted
    /// amounts) pay the protocol a factor of the achieved improvement.
    /// The fee is taken in `sell` token for `buy` orders and in `buy`
    /// token for `sell` orders.
    PriceImprovement {
        /// Factor of price improvement the protocol charges as a fee.
        /// Price improvement is the difference between executed price and
        /// limit price or quoted price (whichever is better)
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having been quoted
        /// 1990USDC, their price improvement is 10USDC. A factor of 0.5
        /// requires the solver to pay 5USDC to the protocol for
        /// settling this order.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
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
