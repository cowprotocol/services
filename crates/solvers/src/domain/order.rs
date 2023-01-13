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
pub enum Class {
    Market,
    Limit,
    Liquidity,
}

/// An order that is guaranteed to not be a liquidity order.
#[derive(Debug)]
pub struct NonLiquidity<'a>(&'a Order);

impl<'a> NonLiquidity<'a> {
    /// Wraps an order as a user order, returns `None` if the specified order is
    /// not a user order.
    pub fn new(order: &'a Order) -> Option<Self> {
        match order.class {
            Class::Market | Class::Limit => Some(Self(order)),
            Class::Liquidity => None,
        }
    }

    /// Returns a reference to the underlying CoW Protocol order.
    pub fn get(&self) -> &'a Order {
        self.0
    }
}
