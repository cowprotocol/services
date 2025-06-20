use {
    super::{Score, Solution},
    crate::infra,
    std::sync::Arc,
};

#[derive(Clone)]
pub struct Participant<State = Ranked> {
    solution: Solution,
    driver: Arc<infra::Driver>,
    state: State,
}

#[derive(Clone)]
pub struct Unranked;
pub enum Ranked {
    Winner,
    NonWinner,
    Filtered,
}

impl<T> Participant<T> {
    pub fn solution(&self) -> &Solution {
        &self.solution
    }

    pub fn set_computed_score(&mut self, score: Score) {
        self.solution.computed_score = Some(score);
    }

    pub fn driver(&self) -> &Arc<infra::Driver> {
        &self.driver
    }
}

impl Participant<Unranked> {
    pub fn new(solution: Solution, driver: Arc<infra::Driver>) -> Self {
        Self {
            solution,
            driver,
            state: Unranked,
        }
    }

    pub fn rank(self, rank: Ranked) -> Participant<Ranked> {
        Participant::<Ranked> {
            state: rank,
            solution: self.solution,
            driver: self.driver,
        }
    }
}

impl Participant<Ranked> {
    pub fn is_winner(&self) -> bool {
        matches!(self.state, Ranked::Winner)
    }

    pub fn was_filtered(&self) -> bool {
        matches!(self.state, Ranked::Filtered)
    }
}
