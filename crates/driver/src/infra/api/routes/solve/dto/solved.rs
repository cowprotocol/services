use {
    crate::{
        domain::{competition, eth},
        infra::Solver,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Solved {
    pub fn new(solved: competition::Solved, solver: &Solver) -> Self {
        let solution = Solution::new(0, solved, solver);
        Self {
            solutions: vec![solution],
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solved {
    solutions: Vec<Solution>,
}

impl Solution {
    pub fn new(id: u64, solved: competition::Solved, solver: &Solver) -> Self {
        Self {
            id,
            score: solved.score.into(),
            submission_address: solver.address().into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    id: u64,
    #[serde_as(as = "serialize::U256")]
    score: eth::U256,
    submission_address: eth::H160,
}
