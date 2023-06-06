use {
    crate::{
        domain::competition::solution::Settlement,
        infra::{observe, solver::Solver},
    },
    futures::{future::select_ok, FutureExt},
};

pub use crate::boundary::mempool::{Config, GlobalTxPool, HighRisk, Kind, Mempool};

/// Publish a settlement to the mempools and wait until it is confirmed.
pub async fn execute(
    mempools: &[Mempool],
    solver: &Solver,
    settlement: Settlement,
) -> Result<(), Error> {
    if mempools.is_empty() {
        return Err(Error::AllMempoolsFailed);
    }
    select_ok(mempools.iter().map(|mempool| {
        let settlement = settlement.clone();
        async move {
            let result = mempool.execute(solver, settlement).await;
            if let Err(err) = result.as_ref() {
                observe::mempool_failed(mempool, err);
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
