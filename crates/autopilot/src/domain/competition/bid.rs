pub use state::{RankType, Unscored};
use {
    super::Score,
    crate::{domain::competition::Solution, infra},
    ::winner_selection::state,
    std::sync::Arc,
    tracing::instrument,
};

pub type Scored = state::Scored<Score>;
pub type Ranked = state::Ranked<Score>;

/// A solver's auction bid, which includes solution and corresponding driver
/// data, progressing through the winner selection process.
///
/// It uses the type-state pattern to enforce correct state
/// transitions at compile time. The state parameter tracks progression through
/// three phases:
///
/// 1. **Unscored**: Initial state when the solution is received from the driver
/// 2. **Scored**: After computing surplus and fees for the solution
/// 3. **Ranked**: After winner selection determines if this is a winner
#[derive(Clone)]
pub struct Bid<State = Ranked> {
    solution: Solution,
    driver: Arc<infra::Driver>,
    state: State,
}

impl<T> Bid<T> {
    pub fn solution(&self) -> &Solution {
        &self.solution
    }

    pub fn driver(&self) -> &Arc<infra::Driver> {
        &self.driver
    }
}

impl<State> state::HasState for Bid<State> {
    type Next<NewState> = Bid<NewState>;
    type State = State;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState> {
        Bid {
            solution: self.solution,
            driver: self.driver,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Bid<Unscored> {
    #[instrument(skip_all, fields(driver = driver.name))]
    pub fn new(solution: Solution, driver: Arc<infra::Driver>) -> Self {
        Self {
            solution,
            driver,
            state: Unscored,
        }
    }
}
