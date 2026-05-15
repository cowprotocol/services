pub use state::{RankType, Unscored};
use {
    super::Score,
    crate::{domain::competition::Solution, infra},
    ::winner_selection::state,
    std::sync::Arc,
};

pub type Scored = state::Scored<Score>;
pub type Ranked = state::Ranked<Score>;

/// Payload carried by [`Bid`]: the solution plus its originating driver.
/// Accessible directly through the bid via [`winner_selection::Bid`]'s
/// `Deref` impl, so `bid.solution()` and `bid.driver()` keep working.
#[derive(Clone)]
pub struct BidPayload {
    solution: Solution,
    driver: Arc<infra::Driver>,
}

impl BidPayload {
    pub fn new(solution: Solution, driver: Arc<infra::Driver>) -> Self {
        Self { solution, driver }
    }

    pub fn solution(&self) -> &Solution {
        &self.solution
    }

    pub fn driver(&self) -> &Arc<infra::Driver> {
        &self.driver
    }
}

/// A solver's auction bid in the typestate pipeline `Unscored -> Scored ->
/// Ranked`. State transitions are enforced at compile time via
/// [`winner_selection::Bid`].
pub type Bid<State = Ranked> = ::winner_selection::Bid<BidPayload, State>;
