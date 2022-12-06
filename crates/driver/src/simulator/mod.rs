use {crate::logic::eth, thiserror::Error};

mod node;
mod tenderly;

// TODO I would like to move the gas estimation behavior into driver but reuse
// the scoring logic from shared

#[derive(Debug, Error)]
#[error("simulation error")]
pub struct Error;

/// Ethereum transaction simulator.
#[derive(Debug)]
pub struct Simulator(Inner);

// TODO I think in the future this will be implemented in terms of
// logic::settlement::Settlement, which will wrap an eth::Tx and contain the
// logic for encoding a Solution into the Tx. Then, the fact that two-step
// access list simulation is needed will become more intuitive, since it's
// obvious that we're simulating a settlement contract call.
impl Simulator {
    /// Simulate a transaction on the Ethereum network.
    pub async fn simulate(&self, tx: &eth::Tx) -> Result<eth::Simulation, Error> {
        // TODO This should do the multi-step estimation
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
    async fn access_list(&self, tx: &eth::Tx) -> eth::AccessList {
        // The driver provides the users with an option of doing access list simulation
        // on Tenderly because the eth_createAccessList endpoint seems to not be
        // very widely supported yet. When that endpoint becomes more common, it
        // might make sense to drop Tenderly entirely.
        match self {
            Self::Tenderly { tenderly, .. } => tenderly.access_list(tx).await,
            Self::Node(node) => node.access_list(tx).await,
        }
    }

    async fn gas(&self, tx: &eth::Tx, access_list: &eth::AccessList) -> eth::Gas {
        // Gas estimation is always done using the node because it's faster that way and
        // the driver has a connection to a node anyway.
        let node = match self {
            Self::Tenderly { node, .. } => node,
            Self::Node(node) => node,
        };
        node.gas(tx, access_list).await
    }
}
