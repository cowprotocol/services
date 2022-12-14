use {
    crate::logic::{competition::solution, eth},
    thiserror::Error,
};

mod ethereum;
mod tenderly;

#[derive(Debug, Error)]
#[error("simulation error")]
pub struct Error;

/// Ethereum transaction simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

impl Simulator {
    /// Simulate an onchain settlement on the Ethereum network.
    pub async fn simulate(
        &self,
        _settlement: &solution::Settlement,
    ) -> Result<eth::Simulation, Error> {
        // TODO This should do the two-step access list generation and gas estimation
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
    async fn access_list(&self, settlement: &solution::Settlement) -> eth::AccessList {
        match self {
            Self::Tenderly(tenderly) => tenderly.access_list(settlement).await,
            Self::Ethereum(eth) => eth.access_list(settlement).await,
        }
    }

    async fn gas(
        &self,
        settlement: &solution::Settlement,
        access_list: &eth::AccessList,
    ) -> eth::Gas {
        match self {
            Self::Tenderly(tenderly) => tenderly.gas(settlement, access_list).await,
            Self::Ethereum(eth) => eth.gas(settlement, access_list).await,
        }
    }
}
