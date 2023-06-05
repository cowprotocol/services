use {
    super::{trade, Error, Solution},
    crate::{
        boundary,
        domain::{
            competition,
            competition::{order, solution},
            eth,
        },
        infra::{blockchain::Ethereum, Simulator},
        util,
    },
    bigdecimal::Signed,
    futures::future::try_join_all,
    rand::Rng,
    std::collections::{HashMap, HashSet},
};

/// A transaction calling into our settlement contract on the blockchain, ready
/// to be published to the blockchain.
///
/// Currently, this represents a wrapper around the [`boundary::Settlement`]
/// concept from the shared part of the codebase. This isn't well-defined
/// enough, it's an intermediate state between a solution and an onchain
/// settlement. The intention with this type is to represent the settlement
/// transaction itself, not an intermediate state.
///
/// This type enforces the following rules:
///
/// - Simulation: the settlement has been simulated without reverting, including
///   the case where no interactions were internalized. Additionally the solver
///   account is known to have sufficient Ether to execute the transaction.
/// - Asset flow: the sum of tokens into and out of the settlement are
///   non-negative, meaning that the solver doesn't take any tokens out of the
///   settlement contract.
/// - Internalization: internalized interactions only use trusted tokens.
///
/// Publishing a settlement which violates these rules would result in slashing
/// for the solver (earning reduced rewards). Enforcing these rules ensures that
/// the settlement can be broadcast safely with high confidence that it will not
/// be reverted and that it will not result in slashing for the solver.
#[derive(Debug, Clone)]
pub struct Settlement {
    pub id: Id,
    /// Necessary for the boundary integration, to allow executing settlements.
    pub boundary: boundary::Settlement,
    /// The access list used by the settlement.
    pub access_list: eth::AccessList,
    /// The gas parameters used by the settlement.
    pub gas: Gas,

    /// See the [`Settlement::solutions`] method.
    solutions: HashMap<solution::Id, Solution>,
}

impl Settlement {
    /// Encode a solution into an onchain settlement.
    pub(super) async fn encode(
        solution: competition::Solution,
        auction: &competition::Auction,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Self, Error> {
        // For a settlement to be valid, the solution has to respect some rules which
        // would otherwise lead to slashing. Check those rules first.

        // Asset flow rule: check that the sum of tokens entering the settlement is not
        // less than the sum of tokens exiting the settlement.
        let mut flow: HashMap<eth::TokenAddress, num::BigInt> = Default::default();

        // Interaction inputs represent flow out of the contract, i.e. negative flow.
        for input in solution
            .interactions
            .iter()
            .flat_map(|interaction| interaction.inputs())
        {
            *flow.entry(input.token).or_default() -= util::conv::u256::to_big_int(input.amount);
        }

        // Interaction outputs represent flow into the contract, i.e. positive flow.
        for output in solution
            .interactions
            .iter()
            .flat_map(|interaction| interaction.outputs())
        {
            *flow.entry(output.token).or_default() += util::conv::u256::to_big_int(output.amount);
        }

        // For trades, the sold amounts are always entering the contract (positive
        // flow), whereas the bought amounts are always exiting the contract
        // (negative flow).
        for trade in solution.trades.iter() {
            let trade::Execution { sell, buy } = trade.execution(&solution)?;
            *flow.entry(sell.token).or_default() += util::conv::u256::to_big_int(sell.amount);
            // Within the settlement contract, the orders which buy ETH are wrapped into
            // WETH, and hence contribute to WETH flow.
            *flow.entry(buy.token.wrap(solution.weth)).or_default() -=
                util::conv::u256::to_big_int(buy.amount);
        }

        if flow.values().any(|v| v.is_negative()) {
            return Err(Error::AssetFlow);
        }

        // Internalization rule: check that internalized interactions only use trusted
        // tokens.
        if !solution
            .interactions
            .iter()
            .filter(|interaction| interaction.internalize())
            .all(|interaction| {
                interaction
                    .inputs()
                    .iter()
                    .all(|asset| auction.is_trusted(asset.token))
            })
        {
            return Err(Error::UntrustedInternalization);
        }

        // Encode the solution into a settlement.
        let boundary = boundary::Settlement::encode(eth, &solution, auction).await?;
        Self::new(
            Id::random(),
            [(solution.id, solution)].into(),
            boundary,
            eth,
            simulator,
        )
        .await
    }

    /// Create a new settlement and ensure that it is valid.
    async fn new(
        id: Id,
        solutions: HashMap<solution::Id, Solution>,
        settlement: boundary::Settlement,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Self, Error> {
        // The settlement contract will fail if the receiver is a smart contract.
        // Because of this, if the receiver is a smart contract and we try to
        // estimate the access list, the access list estimation will also fail.
        //
        // This failure happens because the Ethereum protocol sets a hard gas limit
        // on transferring ETH into a smart contract, which some contracts exceed unless
        // the access list is already specified.

        // The solution is to do access list estimation in two steps: first, simulate
        // moving 1 wei into every smart contract to get a partial access list, and then
        // use that partial access list to calculate the final access list.
        let user_trades = solutions
            .values()
            .flat_map(|solution| solution.user_trades());
        let partial_access_lists = try_join_all(user_trades.map(|trade| async {
            if !trade.order().buys_eth() || !trade.order().pays_to_contract(eth).await? {
                return Ok(Default::default());
            }
            let tx = eth::Tx {
                from: settlement.solver,
                to: trade.order().receiver(),
                value: 1.into(),
                input: Vec::new(),
                access_list: Default::default(),
            };
            Result::<_, Error>::Ok(simulator.access_list(tx).await?)
        }))
        .await?;
        let partial_access_list = partial_access_lists
            .into_iter()
            .fold(eth::AccessList::default(), |acc, list| acc.merge(list));

        // Simulate the settlement and get the access list and gas.
        let (access_list, gas) = Self::simulate(
            id,
            settlement.clone(),
            &partial_access_list,
            eth,
            simulator,
            Internalization::Enable,
        )
        .await?;
        let price = eth.gas_price().await?;
        let gas = Gas::new(gas, price);

        // Ensure that the solver has sufficient balance for the settlement to be mined.
        if eth.balance(settlement.solver).await? < gas.required_balance() {
            return Err(Error::InsufficientBalance);
        }

        // Is at least one interaction internalized?
        if solutions
            .values()
            .flat_map(|solution| solution.interactions.iter())
            .any(|interaction| interaction.internalize())
        {
            // Some rules which are enforced by the settlement contract for non-internalized
            // interactions are not enforced for internalized interactions (in order to save
            // gas). However, publishing a settlement with interactions that violate
            // these rules constitutes a punishable offense for the solver, even if
            // the interactions are internalized. To ensure that this doesn't happen, check
            // that the settlement simulates even when internalizations are disabled.
            Self::simulate(
                id,
                settlement.clone(),
                &partial_access_list,
                eth,
                simulator,
                Internalization::Disable,
            )
            .await?;
        }

        Ok(Self {
            id,
            solutions,
            boundary: settlement,
            access_list,
            gas,
        })
    }

    /// Simulate executing this settlement on the blockchain. This process
    /// ensures that the settlement does not revert, and calculates the
    /// access list and gas needed to settle the solution.
    async fn simulate(
        id: Id,
        settlement: boundary::Settlement,
        partial_access_list: &eth::AccessList,
        eth: &Ethereum,
        simulator: &Simulator,
        internalization: Internalization,
    ) -> Result<(eth::AccessList, eth::Gas), Error> {
        // Add the partial access list to the settlement tx.
        let tx = settlement
            .tx(id, eth.contracts().settlement(), internalization)
            .set_access_list(partial_access_list.to_owned());

        // Simulate the full access list, passing the partial access
        // list into the simulation.
        let access_list = simulator.access_list(tx.clone()).await?;
        let tx = tx.set_access_list(access_list.clone());

        // Simulate the settlement using the full access list and get the gas used.
        let gas = simulator.gas(tx).await?;

        Ok((access_list, gas))
    }

    /// The onchain transaction representing this settlement.
    pub fn tx(
        &self,
        contract: &contracts::GPv2Settlement,
        internalization: Internalization,
    ) -> eth::Tx {
        self.boundary.tx(self.id, contract, internalization)
    }

    // TODO(#1494): score() should be defined on Solution rather than Settlement.
    /// Calculate the score for this settlement.
    pub fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
    ) -> Result<super::Score, boundary::Error> {
        self.boundary.score(eth, auction, self.gas.estimate)
    }

    // TODO(#1478): merge() should be defined on Solution rather than Settlement.
    /// Merge another settlement into this settlement.
    ///
    /// Merging settlements results in a score that can be anything due to the
    /// fact that contracts can do basically anything, but in practice it can be
    /// assumed that the score will be at least equal to the sum of the scores
    /// of the merged settlements.
    pub async fn merge(
        &self,
        other: &Self,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Self, Error> {
        // The solver must be the same for both settlements.
        if self.boundary.solver != other.boundary.solver {
            return Err(Error::DifferentSolvers);
        }

        // Merge the settlements.
        let mut solutions = self.solutions.clone();
        solutions.extend(
            other
                .solutions
                .iter()
                .map(|(id, solution)| (*id, solution.clone())),
        );
        Self::new(
            self.id,
            solutions,
            self.boundary.clone().merge(other.boundary.clone())?,
            eth,
            simulator,
        )
        .await
    }

    /// The solutions encoded in this settlement. This is a [`HashSet`] because
    /// multiple solutions can be encoded in a single settlement due to
    /// merging. See [`Self::merge`].
    pub fn solutions(&self) -> HashSet<super::Id> {
        self.solutions.keys().copied().collect()
    }

    /// Address of the solver which generated this settlement.
    pub fn solver(&self) -> eth::Address {
        self.boundary.solver
    }

    /// The settled user orders.
    pub fn orders(&self) -> HashSet<order::Uid> {
        self.solutions
            .values()
            .flat_map(|solution| solution.user_trades().map(|trade| trade.order().uid))
            .collect()
    }
}

/// A unique settlement ID. This ID is encoded as part of the calldata of the
/// settlement transaction, after the regular calldata. It's used by the
/// protocol to match onchain transactions to corresponding solutions. This is
/// the ID that the protocol uses to refer both to the settlement and to the
/// winning solution. This ID is randomly generated by the driver.
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
