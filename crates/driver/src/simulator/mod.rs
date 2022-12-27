use crate::{blockchain, domain::eth, Ethereum};

pub mod tenderly;

/// Ethereum transaction simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

impl Simulator {
    /// Simulate transactions on [Tenderly](https://tenderly.co/).
    pub fn tenderly(config: tenderly::Config) -> Self {
        Self(Inner::Tenderly(tenderly::Tenderly::new(config)))
    }

    /// Simulate transactions using the Ethereum RPC API.
    pub fn ethereum(eth: Ethereum) -> Self {
        Self(Inner::Ethereum(eth))
    }

    /// Simulate the access list needed by a transaction. Return a new
    /// transaction with an updated access list.
    pub async fn access_list(&self, tx: eth::Tx) -> Result<eth::Tx, Error> {
        let access_list = match &self.0 {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx.clone(), tenderly::GenerateAccessList::Yes)
                    .await?
                    .access_list
            }
            Inner::Ethereum(ethereum) => ethereum.create_access_list(tx.clone()).await?,
        };
        Ok(tx.merge_access_list(access_list))
    }

    /// Simulate the gas needed by a transaction.
    pub async fn gas(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        Ok(match &self.0 {
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

#[derive(Debug)]
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
