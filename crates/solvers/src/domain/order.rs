//! The domain object representing a CoW Protocol order.

use crate::domain::eth;

/// A CoW Protocol order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub class: Class,
}

/// UID of an order.
#[derive(Debug, Clone, Copy)]
pub struct Uid(pub [u8; 56]);

/// The trading side of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// An order with a fixed buy amount and maximum sell amount.
    Buy,
    /// An order with a fixed sell amount and a minimum buy amount.
    Sell,
}

/// The order classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Market,
    Limit,
    Liquidity,
}

/// A user order marker type.
pub struct UserOrder(Order);

impl UserOrder {
    /// Wraps an order as a user order, returns `Err` with the original order if
    /// it is not a user order.
    pub fn new(order: Order) -> Result<Self, Order> {
        match order.class {
            Class::Market | Class::Limit => Ok(Self(order)),
            Class::Liquidity => Err(order),
        }
    }
}
