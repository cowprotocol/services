use {
    crate::{
        domain::competition::solution::Settlement,
        infra::{observe, solver::Solver},
    },
    futures::{future::select_ok, FutureExt},
};

pub use crate::boundary::mempool::{Config, GlobalTxPool, HighRisk, Kind, Mempool};

/// Publish a settlement to the mempools. Wait until it is confirmed in the
/// background.
pub fn execute(mempools: &[Mempool], solver: &Solver, settlement: &Settlement) {
    if mempools.is_empty() {
        observe::no_mempools();
        return;
    }
    tokio::spawn(select_ok(mempools.iter().cloned().map(|mempool| {
        let solver = solver.clone();
        let settlement = settlement.clone();
        async move {
            let result = mempool.execute(&solver, settlement).await;
            observe::mempool_executed(solver.name(), &mempool, &result);
            result
        }
        .boxed()
    })));
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("all mempools failed to send the transaction")]
    AllMempoolsFailed,
}
