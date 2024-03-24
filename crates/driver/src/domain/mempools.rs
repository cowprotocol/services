use {
    super::{competition, eth},
    crate::{
        domain::{competition::solution::Settlement, eth::TxStatus},
        infra::{self, observe, solver::Solver, Ethereum},
    },
    ethrpc::current_block::into_stream,
    futures::{future::select_ok, FutureExt, StreamExt},
    thiserror::Error,
    tracing::Instrument,
};

/// Factor by how much a transaction fee needs to be increased to override a
/// pending transaction at the same nonce.
const GAS_PRICE_BUMP: f64 = 1.125;

/// The gas amount required to cancel a transaction.
const CANCELLATION_GAS_AMOUNT: u64 = 21000;

/// The mempools used to execute settlements.
#[derive(Debug, Clone)]
pub struct Mempools {
    mempools: Vec<infra::Mempool>,
    ethereum: Ethereum,
}

impl Mempools {
    pub fn new(mempools: Vec<infra::Mempool>, ethereum: Ethereum) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self { mempools, ethereum })
        }
    }

    /// Publish a settlement to the mempools.
    pub async fn execute(
        &self,
        solver: &Solver,
        settlement: &Settlement,
    ) -> Result<eth::TxId, Error> {
        let (tx_hash, _remaining_futures) =
            select_ok(self.mempools.iter().cloned().map(|mempool| {
                async move {
                    let result = match &mempool {
                        infra::Mempool::Boundary(mempool) => {
                            mempool.execute(solver, settlement.clone()).await
                        }
                        infra::Mempool::Native(inner) => {
                            self.submit(inner, solver, settlement)
                                .instrument(tracing::info_span!(
                                    "mempool",
                                    kind = inner.to_string()
                                ))
                                .await
                        }
                    };
                    observe::mempool_executed(&mempool, settlement, &result);
                    result
                }
                .boxed()
            }))
            .await?;

        Ok(tx_hash)
    }

    /// Defines if the mempools are configured in a way that guarantees that
    /// settled solution will not revert.
    pub fn revert_protection(&self) -> RevertProtection {
        if self.mempools.iter().any(|mempool| {
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
        // Don't submit risky transactions if revert protection is
        // enabled and the settlement may revert in this mempool.
        if settlement.boundary.revertable()
            && matches!(self.revert_protection(), RevertProtection::Enabled)
            && mempool.may_revert()
        {
            return Err(Error::Disabled);
        }

        let deadline = mempool.config().deadline();
        let tx = eth::Tx {
            // boundary.tx() does not populate the access list
            access_list: settlement.access_list.clone(),
            ..settlement.boundary.tx(
                settlement.auction_id,
                self.ethereum.contracts().settlement(),
                competition::solution::settlement::Internalization::Enable,
            )
        };

        // Instantiate block stream and skip the current block before we submit the
        // settlement. This way we only run iterations in blocks that can potentially
        // include the settlement.
        let mut block_stream = into_stream(self.ethereum.current_block().clone());
        block_stream.next().await;

        let hash = mempool.submit(tx.clone(), settlement.gas, solver).await?;

        // Wait for the transaction to be mined, expired or failing.
        let result = loop {
            // Wait for the next block to be mined or we time out.
            if tokio::time::timeout_at(deadline, block_stream.next())
                .await
                .is_err()
            {
                tracing::info!(?hash, "tx not confirmed in time, cancelling");
                self.cancel(mempool, settlement.gas.price, solver).await?;
                break Err(Error::Expired);
            }
            tracing::debug!(?hash, "checking if tx is confirmed");

            let receipt = self
                .ethereum
                .transaction_status(&hash)
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?hash, ?err, "failed to get transaction status",);
                    TxStatus::Pending
                });
            match receipt {
                TxStatus::Executed => break Ok(hash.clone()),
                TxStatus::Reverted => break Err(Error::Revert(hash.clone())),
                TxStatus::Pending => {
                    // Check if transaction still simulates
                    if let Err(err) = self.ethereum.estimate_gas(tx.clone()).await {
                        if err.is_revert() {
                            tracing::info!(
                                ?hash,
                                ?err,
                                "tx started failing in mempool, cancelling"
                            );
                            self.cancel(mempool, settlement.gas.price, solver).await?;
                            break Err(Error::SimulationRevert);
                        } else {
                            tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
                        }
                    }
                }
            }
        };

        if result.is_err() {
            // Do one last attempt to see if the transaction was confirmed (in case of race
            // conditions or misclassified errors like `OrderFilled` simulation failures).
            if let Ok(TxStatus::Executed) = self.ethereum.transaction_status(&hash).await {
                tracing::info!(?hash, "Found confirmed transaction, ignoring error");
                return Ok(hash);
            }
        }
        result
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
        let gas = competition::solution::settlement::Gas {
            estimate: CANCELLATION_GAS_AMOUNT.into(),
            limit: CANCELLATION_GAS_AMOUNT.into(),
            price: pending * GAS_PRICE_BUMP,
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
    #[error("Strategy disabled for this tx")]
    Disabled,
    #[error("Failed to submit: {0:?}")]
    Other(#[from] anyhow::Error),
}
