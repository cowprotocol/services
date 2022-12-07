use {
    crate::logic::{competition::solution, eth},
    thiserror::Error,
};

mod node;
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
    Tenderly {
        tenderly: tenderly::Tenderly,
        node: node::Node,
    },
    Node(node::Node),
}

impl Inner {
    async fn access_list(&self, settlement: &solution::Settlement) -> eth::AccessList {
        // The driver provides the users with an option of doing access list simulation
        // on Tenderly because the eth_createAccessList endpoint seems to not be
        // very widely supported yet. When that endpoint becomes more common, it
        // might make sense to drop Tenderly entirely.
        match self {
            Self::Tenderly { tenderly, .. } => tenderly.access_list(settlement).await,
            Self::Node(node) => node.access_list(settlement).await,
        }
    }

    async fn gas(
        &self,
        settlement: &solution::Settlement,
        access_list: &eth::AccessList,
    ) -> eth::Gas {
        // Gas estimation is always done using the node because it's faster than
        // Tenderly and the driver has a connection to a node anyway.
        let node = match self {
            Self::Tenderly { node, .. } => node,
            Self::Node(node) => node,
        };
        node.gas(settlement, access_list).await
    }
}
