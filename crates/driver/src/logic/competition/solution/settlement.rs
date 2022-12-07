use {
    super::Approvals,
    crate::{
        boundary,
        logic::{competition, eth},
        node,
        EthNode,
        Simulator,
    },
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
    pub async fn encode(
        node: &EthNode,
        _auction: &competition::Auction,
        solution: competition::Solution,
    ) -> Self {
        let mut settlement = boundary::Settlement::new(solution.prices);
        // TODO No unwrap
        let approvals = Self::filter_approvals(node, &solution.approvals)
            .await
            .unwrap();
        for approval in approvals {
            settlement
                .encoder
                .append_to_execution_plan(boundary::Approval::Approve {
                    token: approval.spender.token.0,
                    spender: approval.spender.address.0,
                });
        }
        // TODO Encode the remaining executions, I believe the auction is needed for
        // this
        Self(settlement)
    }

    /// Filter out approvals which have already been approved.
    async fn filter_approvals(
        node: &EthNode,
        approvals: &Approvals,
    ) -> Result<Vec<eth::Approval>, node::Error> {
        let spenders = approvals.iter().map(|approval| approval.spender);
        let allowances = node
            .allowances(node.settlement_contract().await?, spenders)
            .await?;
        Ok(approvals
            .iter()
            .copied()
            .zip(allowances)
            .filter(|(approval, allowance)| !approval.is_approved(allowance))
            .map(|(approval, _)| approval)
            .collect())
    }

    /// Calculate the score for this settlement. This is the score of the
    /// solution that was encoded in this settlement.
    pub fn score(&self, _simulator: &Simulator) -> Score {
        // TODO This will also call into the boundary because the objective value
        // calculation is tricky and difficult to get right. This is a short-term
        // solution, I'd like to revisit that logic because it seems a bit convoluted
        // and I wonder if we can make it correspond more closely to the descriptions
        // and formulas that we have on docs.cow.fi
        //
        // TODO I intend to do the access list generation and gas estimation in driver
        // though, that will not be part of the boundary
        todo!()
    }
}
