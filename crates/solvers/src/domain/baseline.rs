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
    /// Maximum number of hops that can be considered in a trading path. A value
    /// of 1 indicates that only a direct trade is allowed.
    pub max_hops: NonZeroUsize,
}

impl Baseline {
    /// Finds the optimal trading route for the specified order and liquidity.
    ///
    /// Returns `None` if no trading route can be found.
    fn route<'a>(
        &self,
        order: &order::UserOrder,
        liquidity: &'a [liquidity::Liquidity],
    ) -> Option<Route<'a>> {
        let solver = boundary::baseline::Solver::new(&self.weth, &self.base_tokens, liquidity);
        solver.route(order, self.max_hops)
    }
}

/// A trading route.
pub struct Route<'a> {
    segments: Vec<Segment<'a>>,
}

/// A segment in a trading route.
pub struct Segment<'a> {
    pub liquidity: &'a liquidity::Liquidity,
    // TODO: There is no type-level guarantee here that both `input.token` and
    // `output.token` are valid for the liquidity in this segment. This is
    // unfortunate beacuse this type leaks out of this module (currently into
    // the `boundary::baseline` module) but should no longer need to be `pub`
    // once the `boundary::baseline` module gets refactored into the domain
    // logic.
    pub input: eth::Asset,
    pub output: eth::Asset,
}

impl<'a> Route<'a> {
    pub fn new(segments: Vec<Segment<'a>>) -> Option<Self> {
        if segments.is_empty() {
            return None;
        }
        Some(Self { segments })
    }
}
