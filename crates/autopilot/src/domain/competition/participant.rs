use {super::Score, crate::infra, std::sync::Arc};

#[derive(Clone)]
pub struct Participant<State = Ranked> {
    solution: super::Solution,
    driver: Arc<infra::Driver>,
    state: State,
}

#[derive(Clone)]
pub struct Unscored;

#[derive(Clone)]
pub struct Scored {
    pub(super) score: Score,
}

#[derive(Clone)]
pub struct Ranked {
    pub(super) rank_type: RankType,
    pub(super) score: Score,
}

#[derive(Clone)]
pub enum RankType {
    Winner,
    NonWinner,
    FilteredOut,
}

impl<T> Participant<T> {
    pub fn solution(&self) -> &super::Solution {
        &self.solution
    }

    pub fn driver(&self) -> &Arc<infra::Driver> {
        &self.driver
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

    pub fn with_score(self, score: Score) -> Participant<Scored> {
        Participant {
            solution: self.solution,
            driver: self.driver,
            state: Scored { score },
        }
    }
}

impl Participant<Scored> {
    pub fn score(&self) -> Score {
        self.state.score
    }

    pub fn rank(self, rank_type: RankType) -> Participant<Ranked> {
        Participant {
            solution: self.solution,
            driver: self.driver,
            state: Ranked {
                rank_type,
                score: self.state.score,
            },
        }
    }
}

impl Participant<Ranked> {
    pub fn score(&self) -> Score {
        self.state.score
    }

    pub fn is_winner(&self) -> bool {
        matches!(self.state.rank_type, RankType::Winner)
    }

    pub fn filtered_out(&self) -> bool {
        matches!(self.state.rank_type, RankType::FilteredOut)
    }
}
