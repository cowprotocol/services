pub mod auction;
pub mod order;
pub mod solution;

use {crate::logic::eth, primitive_types::U256};
pub use {
    auction::Auction,
    order::Order,
    solution::{solve, Score, Solution},
};

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Debug, Clone, Copy)]
pub struct Price(pub eth::Ether);

impl From<Price> for U256 {
    fn from(price: Price) -> Self {
        price.0.into()
    }
}

impl From<U256> for Price {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}
