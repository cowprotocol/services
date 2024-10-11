use {super::Solution, crate::infra, std::sync::Arc};

#[derive(Clone)]
pub struct Participant<State = Ranked> {
    solution: Solution,
    driver: Arc<infra::Driver>,
    state: State,
}

#[derive(Clone)]
pub struct Unranked;
pub struct Ranked {
    is_winner: bool,
}

impl<T> Participant<T> {
    pub fn solution(&self) -> &Solution {
        &self.solution
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

    pub fn rank(self, is_winner: bool) -> Participant<Ranked> {
        Participant::<Ranked> {
            state: Ranked { is_winner },
            solution: self.solution,
            driver: self.driver,
        }
    }
}

impl Participant<Ranked> {
    pub fn is_winner(&self) -> bool {
        self.state.is_winner
    }
}
