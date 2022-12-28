use crate::logic::eth;

pub mod ethereum;
pub mod tenderly;

/// Ethereum transaction simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

impl Simulator {
    /// Simulate the access list needed by a transaction. Return a new
    /// transaction with an updated access list.
    pub async fn access_list(&self, tx: eth::Tx) -> Result<eth::Tx, Error> {
        let simulation = match &self.0 {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(&tx, tenderly::GenerateAccessList::Yes)
                    .await?
            }
            Inner::Ethereum(ethereum) => ethereum.simulate(&tx).await,
        };
        Ok(tx.merge_access_list(simulation.access_list))
    }

    /// Simulate the gas needed by a transaction.
    pub async fn gas(&self, tx: &eth::Tx) -> Result<eth::Gas, Error> {
        Ok(match &self.0 {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx, tenderly::GenerateAccessList::No)
                    .await?
                    .gas
            }
            Inner::Ethereum(ethereum) => ethereum.simulate(tx).await.gas,
        })
    }
}

#[derive(Debug)]
enum Inner {
    /// Simulate transactions on [Tenderly](https://tenderly.co/).
    Tenderly(tenderly::Tenderly),
    /// Simulate transactions using the Ethereum RPC API.
    Ethereum(ethereum::Ethereum),
}

#[derive(Debug)]
struct Simulation {
    gas: eth::Gas,
    access_list: eth::AccessList,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tenderly error: {0:?}")]
    Tenderly(#[from] tenderly::Error),
}
