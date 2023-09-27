use {
    crate::{
        domain::competition::solution::Settlement,
        infra::{self, observe, solver::Solver},
    },
    futures::{future::select_ok, FutureExt},
    thiserror::Error,
};

/// The mempools used to execute settlements.
#[derive(Debug, Clone)]
pub struct Mempools(Vec<infra::Mempool>);

impl Mempools {
    pub fn new(mempools: Vec<infra::Mempool>) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self(mempools))
        }
    }

    /// Publish a settlement to the mempools. Wait until it is confirmed in the
    /// background.
    pub fn execute(&self, solver: &Solver, settlement: &Settlement) {
        tokio::spawn(select_ok(self.0.iter().cloned().map(|mempool| {
            let solver = solver.clone();
            let settlement = settlement.clone();
            async move {
                let result = mempool.execute(&solver, settlement.clone()).await;
                observe::mempool_executed(&mempool, &settlement, &result);
                result
            }
            .boxed()
        })));
    }

    /// Defines if the mempools are configured in a way that could lead to
    /// significant costs in case a settlement fails onchain submission.
    pub fn high_risk(&self) -> HighRisk {
        if self.0.iter().any(|mempool| {
            matches!(
                mempool.config().kind,
                infra::mempool::Kind::Public(infra::mempool::HighRisk::Enabled)
            )
        }) {
            HighRisk::Enabled
        } else {
            HighRisk::Disabled
        }
    }
}

#[derive(Debug, Error)]
#[error("no mempools configured, cannot execute settlements")]
pub struct NoMempools;

/// Defines if the mempools are configured in a way that could lead to
/// significant costs in case a settlement fails onchain submission.
#[derive(Debug, Clone, Copy)]
pub enum HighRisk {
    Enabled,
    Disabled,
}
