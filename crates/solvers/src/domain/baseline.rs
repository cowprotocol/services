//! "Baseline" solver implementation.

use crate::{
    boundary,
    domain::{eth, liquidity, order},
};
use std::{collections::HashSet, num::NonZeroUsize};

pub struct Baseline {
    pub weth: eth::WethAddress,
    /// Set of base-tokens for which to always consider when path-finding. This
    /// allows paths of the kind `TOKEN1 -> WETH -> TOKEN2` to be considered
    /// even if there are no orders trading these tokens.
    pub base_tokens: HashSet<eth::TokenAddress>,
    /// Number of additinal hops that can be considered in a trading path. A
    /// value of 1 indicates that only a direct trade is allowed.
    pub hops: NonZeroUsize,
}

/// A trading path.
pub type Route = Vec<()>;

impl Baseline {
    /// Finds the optimal trading route for the specified order and liquidity.
    ///
    /// Returns `None` if no trading route can be found.
    pub fn route(
        &self,
        _order: &order::UserOrder,
        liquidity: &[liquidity::Liquidity],
    ) -> Option<Route> {
        let _solver = boundary::baseline::Solver::new(&self.weth, &self.base_tokens, liquidity);
        todo!()
    }
}
