use crate::logic::competition::{self, order};

/// A trade which executes an order as part of this solution.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

/// A trade which fulfills an order from the auction.
#[derive(Debug)]
pub struct Fulfillment {
    pub order: competition::Order,
    /// The amount executed by this fulfillment. See
    /// [`competition::order::Partial`]. If the order is not partial, the target
    /// amount must equal the amount from the order.
    pub executed: competition::order::TargetAmount,
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug)]
pub struct Jit {
    pub order: order::Jit,
    pub executed: competition::order::TargetAmount,
}
