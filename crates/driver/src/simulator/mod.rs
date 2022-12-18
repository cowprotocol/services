use crate::logic::eth;

pub mod ethereum;
pub mod tenderly;

/// Ethereum transaction simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

impl Simulator {
    /// Simulate the access list needed by a transaction. A partial access list
    /// may already be specified.
    pub async fn access_list(
        &self,
        tx: &eth::Tx,
        partial_access_list: eth::AccessList,
    ) -> Result<eth::AccessList, Error> {
        Ok(match &self.0 {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx, &partial_access_list, tenderly::GenerateAccessList::Yes)
                    .await?
                    .access_list
            }
            Inner::Ethereum(ethereum) => {
                ethereum
                    .simulate(tx, &partial_access_list)
                    .await
                    .access_list
            }
        }
        .merge(partial_access_list))
    }

    /// Simulate the gas needed by a transaction.
    pub async fn gas(
        &self,
        tx: &eth::Tx,
        access_list: &eth::AccessList,
    ) -> Result<eth::Gas, Error> {
        Ok(match &self.0 {
            Inner::Tenderly(tenderly) => {
                tenderly
                    .simulate(tx, access_list, tenderly::GenerateAccessList::No)
                    .await?
                    .gas
            }
            Inner::Ethereum(ethereum) => ethereum.simulate(tx, access_list).await.gas,
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
