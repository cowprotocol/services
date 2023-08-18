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
}

#[derive(Debug, Error)]
#[error("no mempools configured, cannot execute settlements")]
pub struct NoMempools;
