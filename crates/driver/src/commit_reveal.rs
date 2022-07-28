use anyhow::Result;
use model::order::OrderUid;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use solver::{
    settlement::Settlement,
    solver::{Auction, Solver},
};

/// A `SolutionSummary` holds all information solvers are willing to disclose during settlement
/// competition. It does **not** have to include the call data, yet.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct SettlementSummary {
    /// Surplus is denominated in the chain's native token and based off of the auction's external
    /// prices.
    pub surplus: f64,
    /// This is how much gas the solver would like to get reimbursed for executing this solution.
    pub gas_reimbursement: U256,
    /// Orders which would get settled by this solution. Partially fillable orders don't have to be
    /// filled completely to be considered in this list.
    pub settled_orders: Vec<OrderUid>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait CommitRevealSolving: Send + Sync {
    /// Calculates a solution for a given `Auction` but does **not** disclose secret details.
    async fn commit(&self, auction: Auction) -> Result<SettlementSummary>;

    /// Finalizes solution for a previously calculated `SolutionSummary` which can be used to compute
    /// executable call data. If the solver no longer wants to execute the solution it returns
    /// `Ok(None)`.
    async fn reveal(&self, summary: SettlementSummary) -> Result<Option<Settlement>>;
}

// Wraps a legacy `Solver` implementation and makes it compatible with the commit reveal protocol.
// Because RFQ support can not be solved generically the wrapped `Solver` will not be able to opt into
// RFQ orders, yet. A solver would have to support RFQ themselves.
// For now this wrapper is only a compatibility layer to let us use the new driver with existing
// solvers for faster development.
pub struct CommitRevealSolver {
    #[allow(dead_code)]
    solver: Box<dyn Solver>,
}

impl CommitRevealSolver {
    pub fn new(solver: Box<dyn Solver>) -> Self {
        Self { solver }
    }
}

#[async_trait::async_trait]
impl CommitRevealSolving for CommitRevealSolver {
    async fn commit(&self, _auction: Auction) -> Result<SettlementSummary> {
        todo!()
    }

    async fn reveal(&self, _summary: SettlementSummary) -> Result<Option<Settlement>> {
        todo!()
    }
}
