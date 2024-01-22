use {
    super::eth,
    crate::{
        domain::competition::solution::Settlement,
        infra::{self, observe, solver::Solver, Ethereum},
    },
    futures::{future::select_ok, FutureExt},
    thiserror::Error,
    tracing::Instrument,
};

/// The mempools used to execute settlements.
#[derive(Debug, Clone)]
pub struct Mempools(Vec<infra::Mempool>, Ethereum);

impl Mempools {
    pub fn new(mempools: Vec<infra::Mempool>, ethereum: Ethereum) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self(mempools, ethereum))
        }
    }

    /// Publish a settlement to the mempools.
    pub async fn execute(
        &self,
        solver: &Solver,
        settlement: &Settlement,
    ) -> Result<eth::TxId, Error> {
        let auction_id = settlement.auction_id;
        let solver_name = solver.name();

        let (tx_hash, _remaining_futures) = select_ok(self.0.iter().cloned().map(|mempool| {
            async move {
                let result = match &mempool {
                    infra::Mempool::Boundary(mempool) => {
                        mempool.execute(solver, settlement.clone()).await
                    }
                    infra::Mempool::Native(_) => {
                        todo!("implement")
                    }
                };
                observe::mempool_executed(&mempool, settlement, &result);
                result
            }
            .instrument(tracing::info_span!(
                "execute",
                solver = ?solver_name,
                ?auction_id,
            ))
            .boxed()
        }))
        .await?;

        Ok(tx_hash)
    }

    /// Defines if the mempools are configured in a way that guarantees that
    /// /settle'd solution will not revert.
    pub fn revert_protection(&self) -> RevertProtection {
        if self.0.iter().any(|mempool| {
            matches!(
                mempool.config().kind,
                infra::mempool::Kind::Public(infra::mempool::RevertProtection::Disabled)
            )
        }) {
            RevertProtection::Disabled
        } else {
            RevertProtection::Enabled
        }
    }
}

#[derive(Debug, Error)]
#[error("no mempools configured, cannot execute settlements")]
pub struct NoMempools;

/// Defines if the mempools are configured in a way that guarantees that
/// /settle'd solution will not revert.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Mined reverted transaction: {0:?}")]
    Revert(eth::TxId),
    #[error("Simulation started reverting during submission")]
    SimulationRevert,
    #[error("Failed to submit: {0:?}")]
    Other(#[from] anyhow::Error),
}
