use {
    super::{competition, eth},
    crate::{
        domain::{competition::solution::Settlement, eth::FeePerGas},
        infra::{self, observe, solver::Solver, Ethereum},
    },
    ethrpc::current_block::into_stream,
    futures::{future::select_ok, FutureExt, StreamExt},
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
                    infra::Mempool::Native(mempool) => {
                        self.submit(&mempool, solver, settlement).await
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

    async fn submit(
        &self,
        mempool: &infra::mempool::Inner,
        solver: &Solver,
        settlement: &Settlement,
    ) -> Result<eth::TxId, Error> {
        let eth = &self.1;
        let start = std::time::Instant::now();
        let tx = settlement.boundary.tx(
            settlement.auction_id,
            self.1.contracts().settlement(),
            competition::solution::settlement::Internalization::Enable,
        );
        let hash = mempool.submit(tx.clone(), settlement.gas, solver).await?;
        let mut block_stream = into_stream(eth.current_block().clone());
        loop {
            match eth.transaction_receipt(&hash).await.unwrap_or_else(|err| {
                tracing::warn!(
                    "failed to get transaction receipt for tx {:?}: {:?}",
                    hash,
                    err
                );
                None
            }) {
                Some(true) => return Ok(hash.into()),
                Some(false) => return Err(Error::Revert(hash.into())),
                None => {
                    // Check if too late
                    if start.elapsed() >= mempool.config().max_confirm_time {
                        tracing::warn!("tx {:?} not confirmed in time, cancelling", hash,);
                        self.cancel(mempool, settlement.gas.price, solver).await?;
                        return Err(Error::Expired);
                    }

                    // Check if transaction still simulates
                    if let Err(err) = eth.estimate_gas(tx.clone()).await {
                        tracing::warn!("tx started failing in mempool {:?}: {:?}", hash, err);
                        self.cancel(mempool, settlement.gas.price, solver).await?;
                        return Err(Error::SimulationRevert);
                    }
                }
            }

            // Wait for the next block to be mined.
            block_stream.next().await.expect("blockchains never end");
        }
    }

    /// Cancel a pending settlement by sending a transaction to self with a
    /// slightly higher gas price than the existing one.
    async fn cancel(
        &self,
        mempool: &infra::mempool::Inner,
        pending: eth::GasPrice,
        solver: &Solver,
    ) -> Result<(), Error> {
        let cancellation = eth::Tx {
            from: solver.address(),
            to: solver.address(),
            value: 0.into(),
            input: Default::default(),
            access_list: Default::default(),
        };
        let one_gwei: FeePerGas = FeePerGas(1_000_000_000.into());
        let gas = competition::solution::settlement::Gas {
            estimate: 21000.into(),
            limit: 21000.into(),
            price: eth::GasPrice {
                max: pending.max + one_gwei,
                tip: pending.tip + one_gwei,
                base: pending.base,
            },
        };
        mempool.submit(cancellation, gas, solver).await?;
        Ok(())
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
    #[error("Settlement did not get included in time")]
    Expired,
    #[error("Failed to submit: {0:?}")]
    Other(#[from] anyhow::Error),
}
