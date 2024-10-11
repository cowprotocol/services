use {
    super::{encoding, trade::ClearingPrices, Error, Solution},
    crate::{
        domain::{
            competition::{
                self,
                auction,
                order::{self, Side},
                solution::{self, error, Trade},
            },
            eth,
        },
        infra::{blockchain::Ethereum, observe, solver::ManageNativeToken, Simulator},
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
    /// The prepared on-chain transaction for this settlement
    transaction: SettlementTx,
    /// The gas parameters used by the settlement.
    pub gas: Gas,
    solution: Solution,
}

#[derive(Debug, Clone)]
struct SettlementTx {
    /// Transaction with all internalizable interactions omitted
    internalized: eth::Tx,
    /// Full Transaction without internalizing any interactions
    uninternalized: eth::Tx,
    /// Whether this settlement has interactions that could make it revert
    may_revert: bool,
}

impl SettlementTx {
    fn with_access_list(self, access_list: eth::AccessList) -> Self {
        Self {
            internalized: self.internalized.set_access_list(access_list.clone()),
            uninternalized: self.uninternalized.set_access_list(access_list),
            ..self
        }
    }
}

impl Settlement {
    /// Encode a solution into an onchain settlement.
    pub(super) async fn encode(
        solution: competition::Solution,
        auction: &competition::Auction,
        eth: &Ethereum,
        simulator: &Simulator,
        solver_native_token: ManageNativeToken,
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
        let tx = SettlementTx {
            internalized: encoding::tx(
                auction,
                &solution,
                eth.contracts(),
                solution.approvals(eth, Internalization::Enable).await?,
                Internalization::Enable,
                solver_native_token,
            )?,
            uninternalized: encoding::tx(
                auction,
                &solution,
                eth.contracts(),
                solution.approvals(eth, Internalization::Disable).await?,
                Internalization::Disable,
                solver_native_token,
            )?,
            may_revert: solution.revertable(),
        };
        Self::new(auction.id().unwrap(), solution, tx, eth, simulator).await
    }

    /// Create a new settlement and ensure that it is valid.
    async fn new(
        auction_id: auction::Id,
        solution: Solution,
        transaction: SettlementTx,
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
        let partial_access_lists = try_join_all(solution.user_trades().map(|trade| async {
            if !trade.order().buys_eth() || !trade.order().pays_to_contract(eth).await? {
                return Ok(Default::default());
            }
            let tx = eth::Tx {
                from: solution.solver().address(),
                to: trade.order().receiver(),
                value: 1.into(),
                input: Default::default(),
                access_list: Default::default(),
            };
            Result::<_, Error>::Ok(simulator.access_list(&tx).await?)
        }))
        .await?;
        let partial_access_list = partial_access_lists
            .into_iter()
            .fold(eth::AccessList::default(), |acc, list| acc.merge(list));

        // Simulate the settlement and get the access list and gas.
        let (access_list, gas) = Self::simulate(
            transaction.internalized.clone(),
            &partial_access_list,
            eth,
            simulator,
        )
        .await?;
        let price = eth.gas_price().await?;
        let gas = Gas::new(gas, eth.block_gas_limit(), price)?;

        // Ensure that the solver has sufficient balance for the settlement to be mined.
        if eth.balance(solution.solver().address()).await? < gas.required_balance() {
            return Err(Error::SolverAccountInsufficientBalance(
                gas.required_balance(),
            ));
        }

        // Is at least one interaction internalized?
        if solution
            .interactions()
            .iter()
            .any(|interaction| interaction.internalize())
        {
            // Some rules which are enforced by the settlement contract for non-internalized
            // interactions are not enforced for internalized interactions (in order to save
            // gas). However, publishing a settlement with interactions that violate
            // these rules constitutes a punishable offense for the solver, even if
            // the interactions are internalized. To ensure that this doesn't happen, check
            // that the settlement simulates even when internalizations are disabled.
            Self::simulate(
                transaction.uninternalized.clone(),
                &partial_access_list,
                eth,
                simulator,
            )
            .await?;
        }

        Ok(Self {
            auction_id,
            solution,
            transaction: transaction.with_access_list(access_list),
            gas,
        })
    }

    /// Simulate executing this settlement on the blockchain. This process
    /// ensures that the settlement does not revert, and calculates the
    /// access list and gas needed to settle the solution.
    async fn simulate(
        tx: eth::Tx,
        partial_access_list: &eth::AccessList,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<(eth::AccessList, eth::Gas), Error> {
        // Add the partial access list to the settlement tx.
        let tx = tx.set_access_list(partial_access_list.to_owned());

        // Simulate the full access list, passing the partial access
        // list into the simulation.
        let access_list = simulator.access_list(&tx).await?;
        let tx = tx.set_access_list(access_list.clone());

        // Simulate the settlement using the full access list and get the gas used.
        let gas = simulator.gas(&tx).await;

        observe::simulated(eth, &tx, &gas);
        Ok((access_list, gas?))
    }

    /// The calldata for this settlement.
    pub fn transaction(&self, internalization: Internalization) -> &eth::Tx {
        match internalization {
            Internalization::Enable => &self.transaction.internalized,
            Internalization::Disable => &self.transaction.uninternalized,
        }
    }

    /// Whether the settlement contains interactions that could possibly revert
    /// on chain
    pub fn may_revert(&self) -> bool {
        self.transaction.may_revert
    }

    /// Score as defined per CIP38. Equal to surplus + protocol fees.
    pub fn score(
        &self,
        prices: &auction::Prices,
        surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
    ) -> Result<eth::Ether, solution::error::Scoring> {
        self.solution
            .scoring(prices, surplus_capturing_jit_order_owners)
    }

    /// The solution encoded in this settlement.
    pub fn solution(&self) -> &super::Id {
        self.solution.id()
    }

    /// Address of the solver which generated this settlement.
    pub fn solver(&self) -> eth::Address {
        self.solution.solver().address()
    }

    /// The settled user orders with their in/out amounts.
    pub fn orders(&self) -> HashMap<order::Uid, competition::Amounts> {
        let log_err = |trade: &Trade, err: error::Math, kind: &str| -> eth::TokenAmount {
            // This should never happen, returning 0 is better than panicking, but we
            // should still alert.
            let msg = format!("could not compute {kind}");
            tracing::error!(?trade, prices=?self.solution.prices, ?err, msg);
            0.into()
        };
        let mut acc: HashMap<order::Uid, competition::Amounts> = HashMap::new();
        for trade in self.solution.market_trades() {
            match trade {
                Trade::Fulfillment(_) => {
                    let prices = ClearingPrices {
                        sell: self.solution.prices[&trade.sell().token.wrap(self.solution.weth)],
                        buy: self.solution.prices[&trade.buy().token.wrap(self.solution.weth)],
                    };
                    let order = competition::Amounts {
                        side: trade.side(),
                        sell: trade.sell(),
                        buy: trade.buy(),
                        executed_sell: trade
                            .sell_amount(&prices)
                            .unwrap_or_else(|err| log_err(trade, err, "sell_amount")),
                        executed_buy: trade
                            .buy_amount(&prices)
                            .unwrap_or_else(|err| log_err(trade, err, "buy_amount")),
                    };
                    acc.insert(trade.uid(), order);
                }
                Trade::Jit(_) => {
                    let order = competition::Amounts {
                        side: trade.side(),
                        sell: trade.sell(),
                        buy: trade.buy(),
                        executed_sell: Self::jit_order_executed_sell(trade)
                            .unwrap_or_else(|err| log_err(trade, err, "sell_amount")),
                        executed_buy: Self::jit_order_executed_buy(trade)
                            .unwrap_or_else(|err| log_err(trade, err, "buy_amount")),
                    };
                    acc.insert(trade.uid(), order);
                }
            }
        }
        acc
    }

    fn jit_order_executed_buy(trade: &Trade) -> Result<eth::TokenAmount, error::Math> {
        Ok(match trade.side() {
            Side::Buy => trade.executed().into(),
            Side::Sell => (trade
                .executed()
                .0
                .checked_add(trade.fee().0)
                .ok_or(error::Math::Overflow)?)
            .checked_mul(trade.buy().amount.0)
            .ok_or(error::Math::Overflow)?
            .checked_div(trade.sell().amount.0)
            .ok_or(error::Math::DivisionByZero)?
            .into(),
        })
    }

    fn jit_order_executed_sell(trade: &Trade) -> Result<eth::TokenAmount, error::Math> {
        Ok(match trade.side() {
            Side::Buy => trade
                .executed()
                .0
                .checked_mul(trade.sell().amount.0)
                .ok_or(error::Math::Overflow)?
                .checked_div(trade.buy().amount.0)
                .ok_or(error::Math::DivisionByZero)?
                .into(),
            Side::Sell => trade
                .executed()
                .0
                .checked_add(trade.fee().0)
                .ok_or(error::Math::Overflow)?
                .into(),
        })
    }

    /// The uniform price vector this settlement proposes
    pub fn prices(&self) -> HashMap<eth::TokenAddress, eth::TokenAmount> {
        self.solution
            .clearing_prices()
            .iter()
            .map(|asset| (asset.token, asset.amount))
            .collect()
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
