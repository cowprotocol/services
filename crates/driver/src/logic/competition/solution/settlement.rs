use {
    crate::{boundary, logic::competition},
    num::{BigRational, ToPrimitive},
};

#[derive(Debug)]
pub struct Settlement(boundary::Settlement);

/// The solution score. This is often referred to as the "objective value".
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Score(BigRational);

impl From<Score> for f64 {
    fn from(score: Score) -> Self {
        score.0.to_f64().expect("value can be represented as f64")
    }
}

impl Settlement {
    /// Encode a solution into an onchain settlement.
    pub fn encode(solution: &competition::Solution, auction: &competition::Auction) -> Self {
        let mut settlement = boundary::Settlement::new(solution.prices);
        for approval in solution.approvals.iter() {
            settlement
                .encoder
                .append_to_execution_plan(boundary::Approval::Approve {
                    token: approval.spender.token.0,
                    spender: approval.spender.address.0,
                });
        }
        // TODO Encode the remaining executions
        todo!()
    }

    pub fn score(&self) -> Score {
        // TODO This will also call into the boundary because the objective value
        // calculation is tricky and difficult to get right. I think we should do it
        // over later because I feel like it might be more confusing than it needs to
        // be, but not now.
        todo!()
    }
}
