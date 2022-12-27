//! "Baseline" solver implementation.

use super::{auction, eth::TokenAddress, solution::Solution};
use std::collections::HashSet;

pub struct Baseline {
    /// Set of base-tokens for which to always consider when path-finding. This
    /// allows paths of the kind `TOKEN1 -> WETH -> TOKEN2` to be considered
    /// even if there are no orders trading these tokens.
    pub base_tokens: HashSet<TokenAddress>,
}

impl Baseline {
    /// Computes all valid solutions for a given auction.
    pub fn solve(auction: auction::Auction) -> Vec<Solution> {
        
    }
}
