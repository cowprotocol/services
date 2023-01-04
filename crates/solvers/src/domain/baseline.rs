//! "Baseline" solver implementation.

use super::{eth::TokenAddress, liquidity, order};
use std::{collections::HashSet, num::NonZeroUsize};

pub struct Baseline {
    /// Set of base-tokens for which to always consider when path-finding. This
    /// allows paths of the kind `TOKEN1 -> WETH -> TOKEN2` to be considered
    /// even if there are no orders trading these tokens.
    pub base_tokens: HashSet<TokenAddress>,
    /// Number of additinal hops that can be considered in a trading path. A
    /// value of 1 indicates that only a direct trade is allowed.
    pub hops: NonZeroUsize,
}

impl Baseline {
    /// Finds the optimal trading route for the specified order and liquidity.
    pub fn route(order: &order::UserOrder, liquidity: &[liquidity::Liquidity]) -> Vec<Solution> {}
}
