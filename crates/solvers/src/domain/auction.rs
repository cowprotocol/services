use crate::domain::{liquidity, order};

/// The auction that the solvers need to find solutions to.
pub struct Auction {
    pub orders: Vec<order::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
}
