use crate::{
    domain::eth,
    infra::blockchain::{self, Ethereum},
};

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
                    .await?
                    .access_list
            }
            Inner::Ethereum(ethereum) => ethereum.create_access_list(tx.clone()).await?,
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
                    .simulate(tx, tenderly::GenerateAccessList::No)
                    .await?
                    .gas
            }
            Inner::Ethereum(ethereum) => ethereum.estimate_gas(tx).await?,
        })
    }
}

#[derive(Debug, Clone)]
enum Inner {
    Tenderly(tenderly::Tenderly),
    Ethereum(Ethereum),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tenderly error: {0:?}")]
    Tenderly(#[from] tenderly::Error),
    #[error("tenderly error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
}
