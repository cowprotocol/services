pub mod auction;
pub mod order;
pub mod quote;
pub mod solution;

pub use {
    auction::Auction,
    order::Order,
    quote::Quote,
    solution::{solve, Score, Solution, SolverTimeout},
};

use crate::domain::eth;

/// The price of a token in wei. This represents how much wei is needed to buy
/// 10**18 of another token.
#[derive(Debug, Clone, Copy)]
pub struct Price(pub eth::Ether);

impl From<Price> for eth::U256 {
    fn from(value: Price) -> Self {
        value.0.into()
    }
}

impl From<eth::U256> for Price {
    fn from(value: eth::U256) -> Self {
        Self(value.into())
    }
}
