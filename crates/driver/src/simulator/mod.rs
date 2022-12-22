use crate::logic::{competition, eth};

pub mod ethereum;
pub mod tenderly;

/// Solution simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

impl Simulator {
    /// Simulate a solution on the Ethereum network.
    pub async fn simulate(
        &self,
        _solution: &competition::Solution,
    ) -> Result<eth::Simulation, Error> {
        // TODO Tackle this after Nick's PR, this should do the full two-step
        // settlement, and the final step should call simulate_fast
        todo!()
    }
}

#[derive(Debug)]
enum Inner {
    /// Simulate transactions on [Tenderly](https://tenderly.co/).
    Tenderly(tenderly::Tenderly),
    /// Simulate transactions on an Ethereum node.
    Ethereum(ethereum::Ethereum),
}

impl Inner {
    /// Simulate a transaction.
    async fn simulate(
        &self,
        tx: &eth::Tx,
        access_list: &eth::AccessList,
    ) -> Result<eth::Simulation, Error> {
        match self {
            Self::Tenderly(tenderly) => tenderly
                .simulate(tx, access_list, tenderly::Speed::Slow)
                .await
                .map_err(Into::into),
            Self::Ethereum(ethereum) => Ok(ethereum.simulate(tx, access_list).await),
        }
    }

    /// Simulate a transaction in fast mode, if supported. If fast mode is not
    /// supported, this method is the same as [`simulate`].
    async fn simulate_fast(
        &self,
        tx: &eth::Tx,
        access_list: &eth::AccessList,
    ) -> Result<eth::Simulation, Error> {
        match self {
            Self::Tenderly(tenderly) => tenderly
                .simulate(tx, access_list, tenderly::Speed::Fast)
                .await
                .map_err(Into::into),
            Self::Ethereum(ethereum) => Ok(ethereum.simulate(tx, access_list).await),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tenderly error: {0:?}")]
    Tenderly(#[from] tenderly::Error),
}
