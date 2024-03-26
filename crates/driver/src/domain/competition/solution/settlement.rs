use {
    super::{trade::ClearingPrices, Error, Solution},
    crate::{
        boundary,
        domain::{
            competition::{self, auction, order, score, solution},
            eth::{self, GasCost},
            mempools,
        },
        infra::{blockchain::Ethereum, observe, Simulator},
    },
    futures::future::try_join_all,
    std::collections::{BTreeSet, HashMap, HashSet},
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
/// - Internalization: internalized interactions only use trusted tokens.
///
/// Publishing a settlement which violates these rules would result in slashing
/// for the solver (earning reduced rewards). Enforcing these rules ensures that
/// the settlement can be broadcast safely with high confidence that it will not
/// be reverted and that it will not result in slashing for the solver.
#[derive(Debug, Clone)]
pub struct Settlement {
    pub auction_id: auction::Id,
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

        // Internalization rule: check that internalized interactions only use trusted
        // tokens.
        let untrusted_tokens = solution
            .interactions
            .iter()
            .filter(|interaction| interaction.internalize())
            .flat_map(|interaction| interaction.inputs())
            .filter(|asset| !auction.tokens().get(asset.token).trusted)
            .map(|asset| asset.token)
            .collect::<BTreeSet<_>>();
        if !untrusted_tokens.is_empty() {
            return Err(Error::NonBufferableTokensUsed(untrusted_tokens));
        }

        // Encode the solution into a settlement.
        let boundary = boundary::Settlement::encode(eth, &solution, auction).await?;
        Self::new(
            auction.id().unwrap(),
            [(solution.id, solution)].into(),
            boundary,
            eth,
            simulator,
        )
        .await
    }

    /// Create a new settlement and ensure that it is valid.
    async fn new(
        auction_id: auction::Id,
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
                input: Default::default(),
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
            auction_id,
            settlement.clone(),
            &partial_access_list,
            eth,
            simulator,
            Internalization::Enable,
        )
        .await?;
        let price = eth.gas_price().await?;
        let gas = Gas::new(gas, eth.block_gas_limit(), price)?;

        // Ensure that the solver has sufficient balance for the settlement to be mined.
        if eth.balance(settlement.solver).await? < gas.required_balance() {
            return Err(Error::SolverAccountInsufficientBalance(
                gas.required_balance(),
            ));
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
                auction_id,
                settlement.clone(),
                &partial_access_list,
                eth,
                simulator,
                Internalization::Disable,
            )
            .await?;
        }

        Ok(Self {
            auction_id,
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
        auction_id: auction::Id,
        settlement: boundary::Settlement,
        partial_access_list: &eth::AccessList,
        eth: &Ethereum,
        simulator: &Simulator,
        internalization: Internalization,
    ) -> Result<(eth::AccessList, eth::Gas), Error> {
        // Add the partial access list to the settlement tx.
        let tx = settlement
            .tx(auction_id, eth.contracts().settlement(), internalization)
            .set_access_list(partial_access_list.to_owned());

        // Simulate the full access list, passing the partial access
        // list into the simulation.
        let access_list = simulator.access_list(tx.clone()).await?;
        let tx = tx.set_access_list(access_list.clone());

        // Simulate the settlement using the full access list and get the gas used.
        let gas = simulator.gas(tx.clone()).await;

        observe::simulated(eth, &tx, &gas);
        Ok((access_list, gas?))
    }

    /// The calldata for this settlement.
    pub fn calldata(
        &self,
        contract: &contracts::GPv2Settlement,
        internalization: Internalization,
    ) -> Vec<u8> {
        self.boundary
            .tx(self.auction_id, contract, internalization)
            .input
            .into()
    }

    fn cip38_score(
        &self,
        auction: &competition::Auction,
    ) -> Result<eth::Ether, solution::error::Scoring> {
        let prices = auction.prices();

        self.solutions
            .values()
            .map(|solution| solution.scoring(&prices))
            .try_fold(eth::Ether(0.into()), |acc, score| {
                score.map(|score| acc + score)
            })
    }

    // TODO(#1494): score() should be defined on Solution rather than Settlement.
    /// Calculate the score for this settlement.
    pub fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
        revert_protection: &mempools::RevertProtection,
    ) -> Result<competition::Score, score::Error> {
        // For testing purposes, calculate CIP38 even before activation
        let score = self.cip38_score(auction);
        tracing::info!(?score, "CIP38 score for settlement: {:?}", self.solutions());

        let score = match self.boundary.score() {
            competition::SolverScore::Solver(score) => {
                let eth = eth.with_metric_label("scoringSolution".into());
                let quality = self.boundary.quality(&eth, auction)?;
                let score = score.try_into()?;
                if score > quality {
                    return Err(score::Error::ScoreHigherThanQuality(score, quality));
                }
                score
            }
            competition::SolverScore::RiskAdjusted(success_probability) => {
                let eth = eth.with_metric_label("scoringSolution".into());
                let quality = self.boundary.quality(&eth, auction)?;
                let gas_cost = self.gas.estimate * auction.gas_price().effective();
                let success_probability = success_probability.try_into()?;
                let objective_value = (quality - gas_cost)?;
                // The cost in case of a revert can deviate non-deterministically from the cost
                // in case of success and it is often significantly smaller. Thus, we go with
                // the full cost as a safe assumption.
                let failure_cost = match revert_protection {
                    mempools::RevertProtection::Enabled => GasCost::zero(),
                    mempools::RevertProtection::Disabled => gas_cost,
                };
                let score = competition::Score::new(
                    auction.score_cap(),
                    objective_value,
                    success_probability,
                    failure_cost,
                )?;
                if score > quality {
                    return Err(score::Error::ScoreHigherThanQuality(score, quality));
                }
                score
            }
            competition::SolverScore::Surplus => score?.0.try_into()?,
        };

        Ok(score)
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
            self.auction_id,
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

    /// The settled user orders with their in/out amounts.
    pub fn orders(&self) -> HashMap<order::Uid, competition::Amounts> {
        self.solutions
            .values()
            .fold(Default::default(), |mut acc, solution| {
                for trade in solution.user_trades() {
                    let order = acc.entry(trade.order().uid).or_default();
                    let prices = ClearingPrices {
                        sell: solution.prices
                            [&trade.order().sell.token.wrap(solution.weth)],
                        buy: solution.prices
                            [&trade.order().buy.token.wrap(solution.weth)],
                    };
                    order.sell = trade.sell_amount(&prices).unwrap_or_else(|err| {
                        // This should never happen, returning 0 is better than panicking, but we
                        // should still alert.
                        tracing::error!(?trade, prices=?solution.prices, ?err, "could not compute sell_amount");
                        0.into()
                    });
                    order.buy = trade.buy_amount(&prices).unwrap_or_else(|err| {
                        // This should never happen, returning 0 is better than panicking, but we
                        // should still alert.
                        tracing::error!(?trade, prices=?solution.prices, ?err, "could not compute buy_amount");
                        0.into()
                    });
                }
                acc
            })
    }

    /// The uniform price vector this settlement proposes
    pub fn prices(&self) -> HashMap<eth::TokenAddress, eth::TokenAmount> {
        self.boundary.clearing_prices()
    }

    /// Settlements have valid notify ID only if they are originated from a
    /// single solution. Otherwise, for merged settlements, no notifications
    /// are sent, therefore, notify id is None.
    pub fn notify_id(&self) -> Option<super::Id> {
        match self.solutions.len() {
            1 => self.solutions.keys().next().copied(),
            _ => None,
        }
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
    /// The gas price (EIP1559) for a settlement transaction.
    pub price: eth::GasPrice,
}

impl Gas {
    /// Computes settlement gas parameters given estimates for gas and gas
    /// price.
    pub fn new(
        estimate: eth::Gas,
        block_limit: eth::Gas,
        price: eth::GasPrice,
    ) -> Result<Self, solution::Error> {
        // We don't allow for solutions to take up more than half of the block's gas
        // limit. This is to ensure that block producers attempt to include the
        // settlement transaction in the next block as long as it is reasonably
        // priced. If we were to allow for solutions very close to the block
        // gas limit, validators may discard the settlement transaction unless it is
        // paying a very high priority fee. This is because the default block
        // building algorithm picks the highest paying transaction whose gas limit
        // will not exceed the remaining space in the block next and ignore transactions
        // whose gas limit exceed the remaining space (without simulating the actual
        // gas required).
        let max_gas = eth::Gas(block_limit.0 / 2);
        if estimate > max_gas {
            return Err(solution::Error::GasLimitExceeded(estimate, max_gas));
        }

        // Specify a different gas limit than the estimated gas when executing a
        // settlement transaction. This allows the transaction to be resilient
        // to small variations in actual gas usage.
        // Also, some solutions can have significant gas refunds that are refunded at
        // the end of execution, so we want to increase gas limit enough so
        // those solutions don't revert with out of gas error.
        const GAS_LIMIT_FACTOR: f64 = 2.0;
        let estimate_with_buffer =
            eth::U256::from_f64_lossy(eth::U256::to_f64_lossy(estimate.into()) * GAS_LIMIT_FACTOR)
                .into();

        Ok(Self {
            estimate,
            limit: std::cmp::min(max_gas, estimate_with_buffer),
            price,
        })
    }

    /// The balance required to ensure settlement execution with the given gas
    /// parameters.
    pub fn required_balance(&self) -> eth::Ether {
        self.limit * self.price.max()
    }
}
