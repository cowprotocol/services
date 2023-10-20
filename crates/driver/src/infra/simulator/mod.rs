use {
    crate::{
        domain::eth,
        infra::blockchain::{self, Ethereum},
    },
    ethcontract::errors::ExecutionError,
    observe::future::Measure,
};

pub mod enso;
pub mod tenderly;

/// Ethereum transaction simulator.
#[derive(Debug, Clone)]
pub struct Simulator {
    inner: Inner,
    disable_access_lists: bool,
    /// If this is [`Some`], every gas estimate will return this fixed
    /// gas value.
    disable_gas: Option<eth::Gas>,
}

/// Configuration of the transaction simulator.
#[derive(Debug)]
pub enum Config {
    Tenderly(tenderly::Config),
    Enso(enso::Config),
}

impl Simulator {
    /// Simulate transactions on [Tenderly](https://tenderly.co/).
    pub fn tenderly(config: tenderly::Config, network_id: eth::NetworkId) -> Self {
        Self {
            inner: Inner::Tenderly(tenderly::Tenderly::new(config, network_id)),
            disable_access_lists: false,
            disable_gas: None,
        }
    }

    /// Simulate transactions using the Ethereum RPC API.
    pub fn ethereum(eth: Ethereum) -> Self {
        Self {
            inner: Inner::Ethereum(eth),
            disable_access_lists: false,
            disable_gas: None,
        }
    }

    /// Simulate transactions using the [Enso Simulator](https://github.com/EnsoFinance/transaction-simulator).
    /// Uses Ethereum RPC API to generate access lists.
    pub fn enso(config: enso::Config, eth: Ethereum) -> Self {
        Self {
            inner: Inner::Enso(enso::Enso::new(config, eth.network().chain), eth),
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
    pub async fn access_list(&self, tx: eth::Tx) -> Result<eth::AccessList, Error> {
        if self.disable_access_lists {
            return Ok(tx.access_list);
        }
        let access_list = match &self.inner {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx.clone(), tenderly::GenerateAccessList::Yes)
                    .await
                    .map_err(with_tx(tx.clone()))?
                    .access_list
            }
            Inner::Ethereum(ethereum) => ethereum
                .create_access_list(tx.clone())
                .await
                .map_err(with_tx(tx.clone()))?,
            Inner::Enso(_, ethereum) => ethereum
                .create_access_list(tx.clone())
                .await
                .map_err(with_tx(tx.clone()))?,
        };
        Ok(tx.access_list.merge(access_list))
    }

    /// Simulate the gas needed by a transaction.
    pub async fn gas(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        if let Some(gas) = self.disable_gas {
            return Ok(gas);
        }
        Ok(match &self.inner {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx.clone(), tenderly::GenerateAccessList::No)
                    .measure("tenderly_simulate_gas")
                    .await
                    .map_err(with_tx(tx))?
                    .gas
            }
            Inner::Ethereum(ethereum) => ethereum
                .estimate_gas(tx.clone())
                .await
                .map_err(with_tx(tx))?,
            Inner::Enso(enso, _) => enso
                .simulate(tx.clone())
                .measure("enso_simulate_gas")
                .await
                .map_err(with_tx(tx))?,
        })
    }
}

#[derive(Debug, Clone)]
enum Inner {
    Tenderly(tenderly::Tenderly),
    Ethereum(Ethereum),
    Enso(enso::Enso, Ethereum),
}

#[derive(Debug, thiserror::Error)]
pub enum SimulatorError {
    #[error("tenderly error: {0:?}")]
    Tenderly(#[from] tenderly::Error),
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
    #[error("enso error: {0:?}")]
    Enso(#[from] enso::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("err: {err:?}, tx: {tx:?}")]
pub struct WithTxError {
    err: SimulatorError,
    tx: eth::Tx,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("basic: {0:?}")]
    Basic(#[from] SimulatorError),
    /// If a transaction reverted, forward that transaction together with the
    /// error.
    #[error("with tx: {0:?}")]
    WithTx(#[from] WithTxError),
}

fn with_tx<E>(tx: eth::Tx) -> impl FnOnce(E) -> Error
where
    E: Into<SimulatorError>,
{
    move |err| {
        let err: SimulatorError = err.into();
        let tx = match &err {
            SimulatorError::Tenderly(tenderly::Error::Http(_)) => None,
            SimulatorError::Tenderly(tenderly::Error::Revert(_)) => Some(tx),
            SimulatorError::Blockchain(blockchain::Error::Method(error))
                if matches!(error.inner, ExecutionError::Revert(_)) =>
            {
                Some(tx)
            }
            SimulatorError::Blockchain(blockchain::Error::Method(_)) => None,
            SimulatorError::Blockchain(blockchain::Error::Web3(inner)) => {
                let error = ExecutionError::from(inner.clone());
                if matches!(error, ExecutionError::Revert(_)) {
                    Some(tx)
                } else {
                    None
                }
            }
            SimulatorError::Blockchain(blockchain::Error::Gas(_)) => None,
            SimulatorError::Blockchain(blockchain::Error::Response(_)) => None,
            SimulatorError::Enso(enso::Error::Http(_)) => None,
            SimulatorError::Enso(enso::Error::Revert(_)) => Some(tx),
        };
        match tx {
            Some(tx) => Error::WithTx(WithTxError { err, tx }),
            None => Error::Basic(err),
        }
    }
}
