use {
    crate::{
        boundary::unbuffered_web3_client,
        domain::{competition, eth, mempools},
        infra,
    },
    ethcontract::dyns::DynWeb3,
};

pub use crate::boundary::mempool::{Config, GlobalTxPool, Kind, RevertProtection, SubmissionLogic};

#[derive(Debug, Clone)]
pub enum Mempool {
    /// Legacy implementation of the mempool, using the shared and solvers crate
    Boundary(crate::boundary::mempool::Mempool),
    /// Driver native mempool implementation
    Native(Box<Inner>),
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boundary(mempool) => write!(f, "Boundary({mempool})"),
            Self::Native(mempool) => write!(f, "Native({mempool})"),
        }
    }
}

impl Mempool {
    pub fn config(&self) -> &Config {
        match self {
            Self::Boundary(mempool) => mempool.config(),
            Self::Native(mempool) => &mempool.config,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Inner {
    transport: DynWeb3,
    config: Config,
}

impl std::fmt::Display for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.kind.format_variant())
    }
}

impl Inner {
    pub fn new(config: Config, transport: DynWeb3) -> Self {
        let transport = match &config.kind {
            Kind::Public(_) => transport,
            // Flashbots Protect RPC fallback doesn't support buffered transport
            Kind::MEVBlocker { url, .. } => unbuffered_web3_client(url),
        };
        Self { config, transport }
    }

    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas: competition::solution::settlement::Gas,
        solver: &infra::Solver,
        nonce: Option<eth::U256>,
    ) -> Result<eth::TxId, mempools::Error> {
        let mut tx = ethcontract::transaction::TransactionBuilder::new(self.transport.clone())
            .from(solver.account().clone())
            .to(tx.to.into())
            .gas_price(ethcontract::GasPrice::Eip1559 {
                max_fee_per_gas: gas.price.max().into(),
                max_priority_fee_per_gas: gas.price.tip().into(),
            })
            .data(tx.input.into())
            .value(tx.value.0)
            .gas(gas.limit.0)
            .access_list(web3::types::AccessList::from(tx.access_list));

        if let Some(nonce) = nonce {
            tx = tx.nonce(nonce)
        };

        tx.send()
            .await
            .map(|result| eth::TxId(result.hash()))
            .map_err(|err| mempools::Error::Other(anyhow::Error::from(err)))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn may_revert(&self) -> bool {
        match &self.config.kind {
            Kind::Public(_) => true,
            Kind::MEVBlocker { .. } => false,
        }
    }
}
