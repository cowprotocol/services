pub use state::{RankType, Unscored};
use {super::Score, crate::infra, ::winner_selection::state, std::sync::Arc};
pub type Scored = state::Scored<Score>;
pub type Ranked = state::Ranked<Score>;

#[derive(Clone)]
pub struct Participant<State = Ranked> {
    solution: Solution,
    driver: Arc<infra::Driver>,
    state: State,
}

impl<T> Participant<T> {
    pub fn solution(&self) -> &super::Solution {
        &self.solution
    }

    pub fn driver(&self) -> &Arc<infra::Driver> {
        &self.driver
    }
}

impl<State> state::WithState for Participant<State> {
    type State = State;
    type WithState<NewState> = Participant<NewState>;

    fn with_state<NewState>(self, state: NewState) -> Self::WithState<NewState> {
        Participant {
            solution: self.solution,
            driver: self.driver,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Participant<Unscored> {
    pub fn new(solution: super::Solution, driver: Arc<infra::Driver>) -> Self {
        Self {
            solution,
            driver,
            state: Unscored,
        }
    }
}
