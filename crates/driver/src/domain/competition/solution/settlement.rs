use {
    crate::{
        boundary,
        domain::{competition, eth},
        infra::{blockchain::Ethereum, Simulator},
    },
    rand::Rng,
    std::collections::HashSet,
};

/// A unique settlement ID. This ID is encoded as part of the calldata of the
/// settlement transaction, and it's used by the protocol to match onchain
/// transactions to corresponding solutions. This is the ID that the protocol
/// uses to refer both to the settlement and the winning solution.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Id(pub u64);

impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Id> for u64 {
    fn from(value: Id) -> Self {
        value.0
    }
}

impl Id {
    pub fn random() -> Self {
        Self(rand::thread_rng().gen())
    }

    pub fn to_be_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

// TODO(#1489): make this abstraction make sense.
/// A transaction calling into our settlement contract on the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
#[derive(Debug, Clone)]
pub(super) struct Settlement {
    solutions: HashSet<super::Id>,
    id: Id,
    boundary: boundary::Settlement,
}

/// Should the interactions be internalized?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internalization {
    /// Internalize interactions which have the `internalize` flag set.
    ///
    /// Since the settlement contract holds balances of multiple tokens, solvers
    /// are in certain cases allowed to "internalize" an AMM interaction, in
    /// order to save on gas.
    ///
    /// <https://docs.cow.fi/off-chain-services/in-depth-solver-specification/output-batch-auction-solutions#using-internal-buffers>
    Enable,
    /// Do not internalize any interactions.
    Disable,
}

impl Settlement {
    /// Encode a solution into an onchain settlement transaction.
    pub async fn encode(
        eth: &Ethereum,
        auction: &competition::Auction,
        solution: &competition::Solution,
    ) -> boundary::Result<Self> {
        let boundary = boundary::Settlement::encode(eth, solution, auction).await?;
        Ok(Self {
            solutions: [solution.id].into(),
            id: Id::random(),
            boundary,
        })
    }

    /// The onchain transaction representing this settlement.
    pub fn tx(
        &self,
        contract: &contracts::GPv2Settlement,
        internalization: Internalization,
    ) -> eth::Tx {
        let mut tx = self.boundary.tx(contract, internalization);
        tx.input.extend(self.id.to_be_bytes());
        tx
    }

    /// The solver which generated this settlement.
    pub fn solver(&self) -> eth::Address {
        self.boundary.solver
    }
}

/// A settlement which has been verified to respect certain fundamental
/// rules of the protocol. In particular:
///
/// - Simulation: the settlement has been simulated without reverting, including
///   the case where no interactions were internalized. Additionally the solver
///   account is known to have sufficient Ether to execute the transaction.
/// - Asset flow: the sum of tokens into and out of the settlement are
///   non-negative, meaning that the solver doesn't take any tokens out of the
///   settlement contract.
/// - Internalization: internalized interactions only use trusted tokens.
///
/// Violating these rules would result in slashing for the solver (earning
/// reduced rewards). After a settlement has been verified, it can be executed
/// onchain.
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

    pub fn id(&self) -> Id {
        self.inner.id
    }

    /// Necessary for the boundary integration, to allow executing settlements.
    pub fn boundary(self) -> boundary::Settlement {
        self.inner.boundary
    }

    /// The solutions encoded in this settlement. This is a [`HashSet`] because
    /// multiple solutions can be encoded in a single settlement due to
    /// merging. See [`Verified::merge`].
    pub fn solutions(&self) -> &HashSet<super::Id> {
        &self.inner.solutions
    }

    /// Merge another settlement into this settlement.
    ///
    /// Merging settlements results in a score that can be anything due to the
    /// fact that contracts can do basically anything, but in practice it can be
    /// assumed that the score will be at least equal to the sum of the scores
    /// of the merged settlements.
    pub async fn merge(
        &self,
        other: &Self,
        contract: &contracts::GPv2Settlement,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Self, super::Error> {
        // The solver must be the same for both settlements.
        if self.inner.boundary.solver != other.inner.boundary.solver {
            return Err(super::Error::DifferentSolvers);
        }

        // If the solutions being merged are not disjoint, the settlements can't be
        // merged.
        if !self.inner.solutions.is_disjoint(&other.inner.solutions) {
            return Err(super::Error::DuplicateSolutions);
        }

        let settlement = self
            .inner
            .boundary
            .clone()
            .merge(other.inner.boundary.clone())?;

        let gas = Gas {
            // After two settlements have been merged, the only verification that
            // needs to be done is simulating the settlement (which also yields the gas
            // needed by the final merged settlement). All other verified rules still
            // hold after the settlements have been merged. See also [`Solution::verify`].
            estimate: simulator
                .gas(settlement.tx(contract, Internalization::Enable))
                .await?,
            ..self.gas
        };

        // Ensure that the solver has sufficient balance for the settlement to be mined.
        if eth.balance(self.inner.solver()).await? < gas.required_balance() {
            return Err(super::Error::InsufficientBalance);
        }

        Ok(Self {
            inner: Settlement {
                id: self.inner.id,
                solutions: self
                    .inner
                    .solutions
                    .union(&other.inner.solutions)
                    .copied()
                    .collect(),
                boundary: settlement,
            },
            access_list: self.access_list.clone().merge(other.access_list.clone()),
            gas,
        })
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
        // fee per gas over 10 blocks, also including the "tip".
        //
        // This is computed as an approximation of:
        //      MAX_FEE_FACTOR = MAX_INCREASE_PER_BLOCK ** DEADLINE_IN_BLOCKS
        //                     = 1.125 ** 10
        //
        // The value of `MAX_GAS_INCREASE_PER_BLOCK` comes from EIP-1559, which
        // dictates that the block base fee can increase by a maximum of 12.5%
        // from one block to another.
        const MAX_FEE_FACTOR: f64 = 3.25;
        let price = eth::U256::from_f64_lossy(
            eth::U256::to_f64_lossy(price.base.into()) * MAX_FEE_FACTOR
                + eth::U256::to_f64_lossy(price.tip.into()),
        )
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

    /// The balance required to ensure settlement execution with the given gas
    /// parameters.
    pub fn required_balance(&self) -> eth::Ether {
        self.limit * self.price
    }
}
