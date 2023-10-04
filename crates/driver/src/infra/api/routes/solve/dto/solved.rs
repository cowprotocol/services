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
#[serde(rename_all = "camelCase")]
pub struct Solved {
    solutions: Vec<Solution>,
}

impl Solution {
    pub fn new(solution_id: u64, solved: competition::Solved, solver: &Solver) -> Self {
        Self {
            solution_id,
            score: solved.score.into(),
            submission_address: solver.address().into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution, used to identify it in subsequent requests
    /// (reveal, settle).
    #[serde_as(as = "serde_with::DisplayFromStr")]
    solution_id: u64,
    #[serde_as(as = "serialize::U256")]
    score: eth::U256,
    submission_address: eth::H160,
}
