use {
    crate::{domain::competition::solution::settlement, infra::solver::Solver},
    futures::{future::select_ok, FutureExt},
};

pub use crate::boundary::mempool::{Config, GlobalTxPool, HighRisk, Kind, Mempool};

pub async fn send(
    mempools: &[Mempool],
    solver: &Solver,
    settlement: settlement::Simulated,
) -> Result<(), Error> {
    select_ok(mempools.iter().map(|mempool| {
        let settlement = settlement.clone();
        async move {
            let result = mempool.send(solver, settlement).await;
            if result.is_err() {
                tracing::warn!(?result, "sending transaction via mempool failed");
            }
            result
        }
        .boxed()
    }))
    .await
    .map_err(|_| Error::AllMempoolsFailed)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("all mempools failed to send the transaction")]
    AllMempoolsFailed,
}
