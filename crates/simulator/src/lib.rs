pub mod encoding;
pub mod ethereum;
pub mod simulation_builder;
mod simulation_encoding;
pub mod state_override_helpers;
pub mod swap_simulator;
pub mod tenderly;
mod utils;

use {
    eth_domain_types::{self as eth, AccessList, Tx},
    http_client::HttpClientFactory,
    observe::future::Measure,
};
pub use {ethereum::Ethereum, tenderly::Tenderly};

#[derive(Debug, Clone)]
pub struct Simulator {
    inner: Inner,
    eth: Ethereum,
    disable_access_lists: bool,
    disable_gas: Option<eth::Gas>,
}

#[derive(Debug, Clone)]
enum Inner {
    Tenderly(Box<tenderly::Tenderly>),
    Ethereum,
}

impl Simulator {
    pub fn tenderly(
        config: &configs::simulator::TenderlyConfig,
        eth: Ethereum,
        http_factory: &HttpClientFactory,
    ) -> Self {
        let eth = eth.with_metric_label("tenderlySimulator".into());
        Self {
            inner: Inner::Tenderly(Box::new(tenderly::Tenderly::new(
                config,
                eth.clone(),
                http_factory,
            ))),
            eth,
            disable_access_lists: false,
            disable_gas: None,
        }
    }

    pub fn ethereum(eth: Ethereum) -> Self {
        let eth = eth.with_metric_label("web3Simulator".into());
        Self {
            inner: Inner::Ethereum,
            eth,
            disable_access_lists: false,
            disable_gas: None,
        }
    }

    /// Disable access list simulation. Some environments, such as less popular
    /// blockchains, don't support access list simulation.
    pub fn disable_access_lists(&mut self) {
        self.disable_access_lists = true;
    }

    /// Disable gas simulation. Useful for testing, but shouldn't be used in
    /// production since it will cause the driver to return invalid scores.
    pub fn disable_gas(&mut self, fixed_gas: eth::Gas) {
        self.disable_gas = Some(fixed_gas);
    }

    /// Simulate the access list needed by a transaction. If the transaction
    /// already has an access list, the returned access list will be a
    /// superset of the existing one.
    pub async fn access_list(&self, tx: &Tx) -> Result<AccessList, Error> {
        if self.disable_access_lists {
            return Ok(tx.access_list.clone());
        }
        let block = self.eth.current_block().borrow().number.into();
        let access_list = match &self.inner {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx.clone(), block, tenderly::GenerateAccessList::Yes)
                    .await
                    .map_err(with(tx.clone(), block))?
                    .access_list
            }
            Inner::Ethereum => self
                .eth
                .create_access_list(tx.clone())
                .await
                .map_err(with(tx.clone(), block))?,
        };
        Ok(tx.access_list.clone().merge(access_list))
    }

    /// Simulate the gas needed by a transaction.
    pub async fn gas(&self, tx: &eth::Tx) -> Result<eth::Gas, Error> {
        if let Some(gas) = self.disable_gas {
            return Ok(gas);
        }
        let block = self.eth.current_block().borrow().number.into();
        Ok(match &self.inner {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx.clone(), block, tenderly::GenerateAccessList::No)
                    .measure("tenderly_simulate_gas")
                    .await
                    .map_err(with(tx.clone(), block))?
                    .gas
            }
            Inner::Ethereum => self
                .eth
                .estimate_gas(tx.clone())
                .await
                .map_err(with(tx.clone(), block))?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SimulatorError {
    #[error("tenderly error: {0:?}")]
    Tenderly(#[from] tenderly::Error),
    #[error("ethereum error: {0:?}")]
    Ethereum(#[from] ethereum::Error),
    #[error("the simulated gas {0} exceeded the gas limit {1} provided in the solution")]
    GasExceeded(eth::Gas, eth::Gas),
}

#[derive(Debug, thiserror::Error)]
#[error("block: {block},  err: {err:?}, tx: {tx:?}")]
pub struct RevertError {
    pub err: SimulatorError,
    pub tx: Tx,
    pub block: eth::BlockNo,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// If a transaction reverted, forward that transaction together with the
    /// error.
    #[error(transparent)]
    Revert(#[from] RevertError),
    /// Any other error that is not related to the underlying transaction
    /// failing.
    #[error(transparent)]
    Other(#[from] SimulatorError),
}

fn with<E>(tx: Tx, block: eth::BlockNo) -> impl FnOnce(E) -> Error
where
    E: Into<SimulatorError>,
{
    move |err| {
        let err: SimulatorError = err.into();
        let tx = match &err {
            SimulatorError::Tenderly(tenderly::Error::Http(_)) => None,
            SimulatorError::Tenderly(tenderly::Error::Revert(_)) => Some(tx.clone()),
            SimulatorError::Tenderly(tenderly::Error::Other(_)) => None,
            SimulatorError::Ethereum(_) => Some(tx.clone()),
            SimulatorError::GasExceeded(..) => Some(tx.clone()),
        };
        match tx {
            Some(tx) => Error::Revert(RevertError {
                err,
                tx: tx.clone(),
                block,
            }),
            None => Error::Other(err),
        }
    }
}
