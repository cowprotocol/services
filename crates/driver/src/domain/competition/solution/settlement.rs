use crate::{
    boundary,
    domain::{competition, eth},
    Ethereum,
};

/// A transaction calling into our settlement contract on the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
#[derive(Debug, Clone)]
pub(super) struct Settlement(boundary::Settlement);

impl Settlement {
    /// Encode a solution into an onchain settlement transaction.
    pub async fn encode(
        eth: &Ethereum,
        auction: &competition::Auction,
        solution: &competition::Solution,
    ) -> anyhow::Result<Self> {
        boundary::Settlement::encode(eth, solution, auction)
            .await
            .map(Self)
    }

    /// The onchain transaction representing this settlement.
    pub fn tx(self) -> eth::Tx {
        self.0.tx()
    }
}

/// A settlement which has been simulated on the blockchain.
#[derive(Debug, Clone)]
pub struct Simulated {
    pub(super) inner: Settlement,
    /// The access list used by the settlement.
    pub access_list: eth::AccessList,
    /// The gas used by the settlement.
    pub gas: eth::Gas,
}

impl Simulated {
    /// Calculate the score for this settlement.
    pub async fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
    ) -> Result<super::Score, boundary::Error> {
        self.inner.0.score(eth, auction, self.gas).await
    }

    /// Necessary for the boundary integration, to allow executing settlements.
    pub fn boundary(self) -> boundary::Settlement {
        self.inner.0
    }
}
