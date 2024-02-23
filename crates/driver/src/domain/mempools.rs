use {
    super::{competition, eth},
    crate::{
        domain::{competition::solution::Settlement, eth::TxStatus},
        infra::{self, observe, solver::Solver, Ethereum},
    },
    anyhow::anyhow,
    ethrpc::current_block::into_stream,
    futures::{future::select_ok, FutureExt, StreamExt},
    std::{collections::HashMap, sync::Arc},
    thiserror::Error,
    tokio::sync::Mutex,
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
    tx_pool: Arc<Mutex<HashMap<eth::Address, PendingTx>>>,
    ethereum: Ethereum,
}

#[derive(Debug, Clone)]
struct PendingTx {
    nonce: eth::U256,
    gas_price: eth::GasPrice,
}

impl Mempools {
    pub fn new(mempools: Vec<infra::Mempool>, ethereum: Ethereum) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self {
                mempools,
                tx_pool: Default::default(),
                ethereum,
            })
        }
    }

    /// Publish a settlement to the mempools.
    pub async fn execute(
        &self,
        solver: &Solver,
        settlement: &Settlement,
    ) -> Result<eth::TxId, Error> {
        // Native solution submission via public mempool may require cancelling
        // a pending transaction and determining a custom nonce.
        let nonce = if let Some(inner) = self.mempools.iter().find_map(|mempool| match mempool {
            infra::Mempool::Boundary(_) => None,
            infra::Mempool::Native(inner) => {
                if matches!(inner.config().kind, infra::mempool::Kind::Public(_)) {
                    Some(inner)
                } else {
                    None
                }
            }
        }) {
            Some(self.cancel_pending_tx(solver, inner).await?)
        } else {
            None
        };

        let (tx_hash, _remaining_futures) =
            select_ok(self.mempools.iter().cloned().map(|mempool| {
                async move {
                    let result = match &mempool {
                        infra::Mempool::Boundary(mempool) => {
                            mempool.execute(solver, settlement.clone()).await
                        }
                        infra::Mempool::Native(inner) => {
                            self.submit(inner, solver, settlement, nonce)
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
        nonce: Option<eth::U256>,
    ) -> Result<eth::TxId, Error> {
        // Don't submit risky transactions if revert protection is
        // enabled and the settlement may revert in this mempool.
        if settlement.boundary.revertable()
            && matches!(self.revert_protection(), RevertProtection::Enabled)
            && mempool.may_revert()
        {
            return Err(Error::Disabled);
        }

        let tx = eth::Tx {
            // boundary.tx() does not populate the access list
            access_list: settlement.access_list.clone(),
            ..settlement.boundary.tx(
                settlement.auction_id,
                self.ethereum.contracts().settlement(),
                competition::solution::settlement::Internalization::Enable,
            )
        };
        let hash = self
            .execute_and_track(mempool, tx.clone(), settlement.gas, solver, nonce)
            .await?;
        let mut block_stream = into_stream(self.ethereum.current_block().clone());
        loop {
            // Wait for the next block to be mined or we time out. Block stream immediately
            // yields the latest block, thus the first iteration starts immediately.
            if tokio::time::timeout_at(mempool.config().deadline(), block_stream.next())
                .await
                .is_err()
            {
                tracing::info!(?hash, "tx not confirmed in time, cancelling");
                self.cancel(mempool, settlement.gas.price, solver, nonce)
                    .await?;
                return Err(Error::Expired);
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
                TxStatus::Executed => return Ok(hash),
                TxStatus::Reverted => return Err(Error::Revert(hash)),
                TxStatus::Pending => {
                    // Check if transaction still simulates
                    if let Err(err) = self.ethereum.estimate_gas(tx.clone()).await {
                        if err.is_revert() {
                            tracing::info!(
                                ?hash,
                                ?err,
                                "tx started failing in mempool, cancelling"
                            );
                            self.cancel(mempool, settlement.gas.price, solver, nonce)
                                .await?;
                            return Err(Error::SimulationRevert);
                        } else {
                            tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
                        }
                    }
                }
            }
        }
    }

    /// Cancel a pending settlement by sending a transaction to self with a
    /// slightly higher gas price than the existing one.
    async fn cancel(
        &self,
        mempool: &infra::mempool::Inner,
        pending: eth::GasPrice,
        solver: &Solver,
        nonce: Option<eth::U256>,
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
        self.execute_and_track(mempool, cancellation, gas, solver, nonce)
            .await?;
        Ok(())
    }

    /// Checks if there is a pending transaction, and if so cancels it.
    /// Returns the nonce to use for the new transaction.
    async fn cancel_pending_tx(
        &self,
        solver: &Solver,
        mempool: &infra::mempool::Inner,
    ) -> Result<eth::U256, Error> {
        let nonce = self
            .ethereum
            .nonce(solver.address())
            .await
            .map_err(|err| Error::Other(anyhow!("Error fetching nonce {}", err)))?;

        if let Some(pending) = self
            .tx_pool
            .lock()
            .await
            .get(&solver.address())
            .filter(|p| p.nonce == nonce)
            .cloned()
        {
            // There is a pending transaction, optimistically cancel it and increment the
            // nonce for the actual settlement.
            tracing::info!(
                ?solver,
                ?nonce,
                "Cancelling pending transaction from previous auction"
            );
            self.cancel(mempool, pending.gas_price, &solver, Some(nonce))
                .await?;
            Ok(nonce + 1)
        } else {
            Ok(nonce)
        }
    }

    async fn execute_and_track(
        &self,
        mempool: &infra::mempool::Inner,
        tx: eth::Tx,
        gas: competition::solution::settlement::Gas,
        solver: &infra::Solver,
        nonce: Option<eth::U256>,
    ) -> Result<eth::TxId, Error> {
        let tx = mempool.submit(tx, gas, solver, nonce).await?;
        tracing::debug!(?tx, ?nonce, "Submitted tx and tracking nonce");

        // Track pending transactions
        if let Some(nonce) = nonce {
            self.tx_pool.lock().await.insert(
                solver.address(),
                PendingTx {
                    nonce,
                    gas_price: gas.price,
                },
            );
        };
        Ok(tx)
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
