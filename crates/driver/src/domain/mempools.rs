use {
    crate::{
        domain::competition::solution::Settlement,
        infra::{self, observe, solver::Solver},
    },
    futures::{future::select_ok, FutureExt},
    thiserror::Error,
    tracing::Instrument,
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
        let auction_id = settlement.auction_id;
        let solver_name = solver.name();
        tokio::spawn(select_ok(self.0.iter().cloned().map(|mempool| {
            let solver = solver.clone();
            let settlement = settlement.clone();
            async move {
                let result = mempool.execute(&solver, settlement.clone()).await;
                observe::mempool_executed(&mempool, &settlement, &result);
                result
            }
            .instrument(tracing::info_span!(
                "execute",
                solver = ?solver_name,
                ?auction_id,
            ))
            .boxed()
        })));
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
