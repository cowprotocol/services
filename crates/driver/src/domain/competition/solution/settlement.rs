use crate::{
    boundary,
    domain::{competition, eth},
    infra::blockchain::Ethereum,
};

/// A transaction calling into our settlement contract on the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
#[derive(Debug, Clone)]
pub(super) struct Settlement {
    id: super::Id,
    boundary: boundary::Settlement,
}

impl Settlement {
    /// Encode a solution into an onchain settlement transaction.
    pub async fn encode(
        eth: &Ethereum,
        auction: &competition::Auction,
        solution: &competition::Solution,
    ) -> anyhow::Result<Self> {
        let boundary = boundary::Settlement::encode(eth, solution, auction).await?;
        Ok(Self {
            id: solution.id,
            boundary,
        })
    }

    /// The onchain transaction representing this settlement.
    pub fn tx(self) -> eth::Tx {
        let mut tx = self.boundary.tx();
        tx.input.extend(self.id.to_be_bytes());
        tx
    }
}

/// A settlement which has been verified to be correct. In particular:
///
/// - Simulation: the settlement has been simulated without reverting.
/// - Asset flow: the sum of tokens into and out of the settlement are
/// non-negative, meaning that the solver doesn't take any tokens out of the
/// settlement contract.
/// - Internalization: internalized interactions only use trusted tokens.
///
/// Such a settlement obeys the rules of the protocol and can be safely
/// broadcast to the Ethereum network.
#[derive(Debug, Clone)]
pub struct Verified {
    pub(super) inner: Settlement,
    /// The access list used by the settlement.
    pub access_list: eth::AccessList,
    /// The gas parameters used by the settlement.
    pub gas: Gas,
}

impl Verified {
    /// Calculate the score for this settlement.
    pub async fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
    ) -> Result<super::Score, boundary::Error> {
        self.inner
            .boundary
            .score(eth, auction, self.gas.estimate)
            .await
    }

    pub fn id(&self) -> super::Id {
        self.inner.id
    }

    /// Necessary for the boundary integration, to allow executing settlements.
    pub fn boundary(self) -> boundary::Settlement {
        self.inner.boundary
    }
}

/// Gas parameters associated with a settlement.
#[derive(Clone, Copy, Debug)]
pub struct Gas {
    /// The gas estimate, in gas units, for executing a settlement transaction.
    pub estimate: eth::Gas,
    /// The gas limit, in gas units, for a settlement transaction. This is
    /// computed by adding a buffer to the gas estimate to allow for small
    /// variations in the actual gas that gets used.
    pub limit: eth::Gas,
    /// The maximum fee per unit of gas for a given settlement.
    pub price: eth::FeePerGas,
}

impl Gas {
    /// Computes settlement gas parameters given estimates for gas and gas
    /// price.
    pub fn new(estimate: eth::Gas, price: eth::GasPrice) -> Self {
        // Compute an upper bound for `max_fee_per_gas` for the given
        // settlement. We multiply a fixed factor of the current base fee per
        // gas, which is chosen to be the maximum possible increase to the base
        // fee per gas over 10 blocks.
        const MAX_FEE_FACTOR: f64 = 3.25;
        let price =
            eth::U256::from_f64_lossy(eth::U256::to_f64_lossy(price.base.into()) * MAX_FEE_FACTOR)
                .into();

        // Specify a different gas limit than the estimated gas when executing a
        // settlement transaction. This allows the transaction to be resilient
        // to small variations in actual gas usage.
        const GAS_LIMIT_FACTOR: f64 = 1.2;
        let limit =
            eth::U256::from_f64_lossy(eth::U256::to_f64_lossy(estimate.into()) * GAS_LIMIT_FACTOR)
                .into();

        Self {
            estimate,
            limit,
            price,
        }
    }

    /// Returns the minimum required balance in Ether that an account needs in
    /// order to afford the specified gas parameters.
    pub fn required_balance(&self) -> eth::Ether {
        self.limit * self.price
    }
}
