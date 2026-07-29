use {
    super::{
        Error,
        Solution,
        encoding,
        trade::{self, ClearingPrices},
    },
    crate::{
        domain::{
            self,
            competition::{
                self,
                auction,
                order::{self},
                solution::{self, Interaction, Trade, error},
            },
        },
        infra::{blockchain::Ethereum, solver::ManageNativeToken},
    },
    alloy::primitives::U256,
    eth_domain_types as eth,
    futures::{FutureExt, future::try_join_all},
    simulator::{self, Simulator},
    std::collections::{BTreeSet, HashMap, HashSet},
    tracing::instrument,
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
#[derive(derive_more::Debug, Clone)]
pub struct Settlement {
    pub auction_id: auction::Id,
    /// The prepared on-chain transaction for this settlement
    transaction: SettlementTx,
    /// The gas parameters used by the settlement.
    pub gas: Gas,
    #[debug(ignore)]
    solution: Solution,
}

#[derive(derive_more::Debug, Clone)]
struct SettlementTx {
    /// Transaction with all internalizable interactions omitted
    internalized: eth::Tx,
    #[debug(ignore)]
    /// Full Transaction without internalizing any interactions
    uninternalized: eth::Tx,
    /// Whether this settlement has interactions that could make it revert
    may_revert: bool,
}

impl SettlementTx {
    fn set_access_list(&mut self, access_list: RequiredAccessList) {
        self.internalized.set_access_list(access_list.0.clone());
        self.uninternalized.set_access_list(access_list.0);
    }
}

impl Settlement {
    /// Encode a solution into an onchain settlement.
    #[instrument(name = "encode_settlement", skip_all)]
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
            .filter(|asset| {
                !auction
                    .tokens()
                    .get(&asset.token)
                    .map(|token| token.trusted)
                    .unwrap_or_default()
            })
            .map(|asset| asset.token)
            .collect::<BTreeSet<_>>();
        if !untrusted_tokens.is_empty() {
            return Err(Error::NonBufferableTokensUsed(untrusted_tokens));
        }

        let (internalized, uninternalized) = futures::try_join!(
            solution.approvals(eth, Internalization::Enable),
            solution.approvals(eth, Internalization::Disable),
        )?;

        // Encode the solution into a settlement.
        let tx = SettlementTx {
            internalized: encoding::tx(
                auction,
                &solution,
                eth.contracts(),
                internalized,
                Internalization::Enable,
                solver_native_token,
            )?,
            uninternalized: encoding::tx(
                auction,
                &solution,
                eth.contracts(),
                uninternalized,
                Internalization::Disable,
                solver_native_token,
            )?,
            may_revert: solution.revertable(),
        };
        Self::new(auction.id().unwrap(), solution, tx, eth, simulator).await
    }

    /// Create a new settlement and ensure that it is valid.
    #[instrument(name = "create_settlement", skip_all)]
    async fn new(
        auction_id: auction::Id,
        solution: Solution,
        mut transaction: SettlementTx,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Self, Error> {
        // <address payable>.transfer(ETH) is allowed to use at most 2300 gas units (
        // see <https://fravoll.github.io/solidity-patterns/secure_ether_transfer.html>).
        // This is not enough when the receiver is a smart contract wallet which does
        // non-trivial work in the `fallback` handler.
        // To support sending native ETH to SC wallets we use access lists which
        // effectively move the cost of accessing storage out of the critical section
        // and into the tx's initial gas cost.
        // While correctly built access lists provide a very minor net cost
        // reduction an access list with unused storage slots increases the cost
        // significantly. Since the risk is high and the reward is very low we only
        // compute access list items which are absolutely necessary for the tx to work.
        //
        // We compute those access lists by using `eth_createAccessList` for a call
        // sending 1 wei to each SC wallet that is supposed to get ETH during the
        // settlement. Those lists get merged and added to the settlement transaction.
        //
        // `Some(..)` means at least one trade strictly requires an access list;
        // `None` means it is purely a gas optimization for this settlement, so a
        // non-revert fetch failure below can be tolerated.
        let partial_access_list: Option<RequiredAccessList> = try_join_all(
            solution
                .user_trades()
                .map(|trade| partial_access_list_for(trade, &solution, eth, simulator)),
        )
        .await?
        .into_iter()
        .flatten()
        .map(|required| required.0)
        .reduce(|acc, list| acc.merge(list))
        .map(RequiredAccessList);

        if let Some(access_list) = partial_access_list {
            transaction.set_access_list(access_list.clone());
        }

        let gas_used_fut = simulator
            .gas(transaction.internalized.clone())
            .inspect(|res| {
                tracing::debug!(
                    block = eth.current_block().borrow().number,
                    transaction = ?transaction.internalized,
                    ?res,
                    "simulated settlement"
                )
            });

        // run everything concurrently to minimize latency added through RPC roundtrips
        let (gas_used, gas_price, solver_eth) = tokio::join!(
            gas_used_fut,
            eth.gas_price(),
            eth.balance(solution.solver().address()),
        );

        // Ensure the solver can cover the gas the node reserves to admit the settlement
        // tx, which is `gas_limit * max_fee_per_gas` at whichever price we submit with.
        let gas = Gas::new(gas_used?, eth.block_gas_limit(), eth.tx_gas_limit())?;
        let required_eth_balance = gas.required_balance(submission_max_fee_per_gas(
            gas_price?.max_fee_per_gas,
            solution
                .gas_fee_override()
                .map(|gas_override| gas_override.max_fee_per_gas),
        ));
        if solver_eth? < required_eth_balance {
            return Err(Error::SolverAccountInsufficientBalance(
                required_eth_balance,
            ));
        }

        Ok(Self {
            auction_id,
            solution,
            transaction,
            gas,
        })
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

    /// Optional gas fee overrides provided by the solver.
    pub fn gas_fee_override(&self) -> Option<super::GasFeeOverride> {
        self.solution.gas_fee_override()
    }

    /// Solution's pre interactions
    pub fn pre_interactions(&self) -> &[domain::Interaction] {
        self.solution.pre_interactions()
    }

    /// Solution's interactions
    pub fn interactions(&self) -> &[Interaction] {
        self.solution.interactions()
    }

    /// Solution's post interactions
    pub fn post_interactions(&self) -> &[domain::Interaction] {
        self.solution.post_interactions()
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
        for trade in &self.solution.trades {
            let order = match trade {
                Trade::Fulfillment(_) => {
                    let prices = ClearingPrices {
                        sell: self.solution.prices
                            [&trade.sell().token.as_erc20(self.solution.weth)],
                        buy: self.solution.prices[&trade.buy().token.as_erc20(self.solution.weth)],
                    };
                    competition::Amounts {
                        side: trade.side(),
                        sell: trade.sell(),
                        buy: trade.buy(),
                        executed_sell: trade
                            .sell_amount(&prices)
                            .unwrap_or_else(|err| log_err(trade, err, "executed_sell")),
                        executed_buy: trade
                            .buy_amount(&prices)
                            .unwrap_or_else(|err| log_err(trade, err, "executed_buy")),
                    }
                }
                Trade::Jit(jit) => competition::Amounts {
                    side: trade.side(),
                    sell: trade.sell(),
                    buy: trade.buy(),
                    executed_sell: jit
                        .executed_sell()
                        .unwrap_or_else(|err| log_err(trade, err, "executed_sell")),
                    executed_buy: jit
                        .executed_buy()
                        .unwrap_or_else(|err| log_err(trade, err, "executed_buy")),
                },
            };
            acc.insert(trade.uid(), order);
        }
        acc
    }

    /// The uniform price vector this settlement proposes.
    ///
    /// Deprecated: only emitted on the `/solve` response so that autopilots
    /// running the previous code can deserialise it during a rolling deploy.
    pub fn prices(&self) -> HashMap<eth::TokenAddress, eth::TokenAmount> {
        self.solution
            .clearing_prices()
            .into_iter()
            .map(|(token, amount)| (token, amount.into()))
            .collect()
    }

    /// Returns true if this settlement's solution has any trades with haircut.
    pub fn has_haircut(&self) -> bool {
        self.solution.has_haircut()
    }
}

/// Access lists that are required when the order buys native ETH and the
/// receiver is a smart-contract.
#[derive(Debug, Clone)]
struct RequiredAccessList(eth::AccessList);

/// Returns the partial access list for a single trade, or `None` if the
/// trade does not buy native ETH or its receiver has no on-chain code.
async fn partial_access_list_for(
    trade: &trade::Fulfillment,
    solution: &Solution,
    eth: &Ethereum,
    simulator: &Simulator,
) -> Result<Option<RequiredAccessList>, Error> {
    if !trade.order().buys_eth() || !trade.order().pays_to_contract(eth).await? {
        return Ok(None);
    }
    Ok(Some(RequiredAccessList(
        simulator
            .access_list_for_eth_transfer(solution.solver().address(), trade.order().receiver())
            .await?,
    )))
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
}

impl Gas {
    /// Computes settlement gas parameters given estimates for gas and gas
    /// price.
    pub fn new(
        estimate: eth::Gas,
        block_limit: eth::Gas,
        tx_gas_limit: eth::Gas,
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
        // Additionally cap by the configured per-tx gas limit. Operators set
        // this per chain (e.g. to EIP-7825's 16,777,215 cap on Mainnet Fusaka)
        // so the mempool can't reject the settlement for exceeding the per-tx
        // ceiling.
        let max_gas = std::cmp::min(eth::Gas(block_limit.0 / eth::U256::from(2)), tx_gas_limit);
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
        let estimate_with_buffer = eth::U256::from(f64::from(estimate.0) * GAS_LIMIT_FACTOR).into();

        Ok(Self {
            estimate,
            limit: std::cmp::min(max_gas, estimate_with_buffer),
        })
    }

    /// The balance required to ensure settlement execution with the given gas
    /// parameters.
    pub fn required_balance(&self, max_fee_per_gas: U256) -> eth::Ether {
        self.limit * max_fee_per_gas.into()
    }
}

/// The `max_fee_per_gas` the settlement will be submitted with, used to size
/// the solver's required balance. Mirrors `apply_gas_fee_override`: with an
/// override we submit at that value, otherwise at the driver estimate doubled
/// to absorb the gas price climbing during submission. The override is the
/// solver's own choice, so we take it as is.
fn submission_max_fee_per_gas(
    driver_max_fee_per_gas: u128,
    override_max_fee_per_gas: Option<u128>,
) -> U256 {
    override_max_fee_per_gas
        .map(U256::from)
        .unwrap_or_else(|| U256::from(driver_max_fee_per_gas).saturating_mul(U256::from(2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gas(value: u64) -> eth::Gas {
        eth::Gas(eth::U256::from(value))
    }

    /// EIP-7825 per-transaction gas cap (2^24 - 1) introduced in Mainnet's
    /// Fusaka hardfork. Used in tests as a representative value for the
    /// configurable `tx_gas_limit` knob on Mainnet.
    const EIP_7825_MAINNET_TX_GAS_CAP: u64 = (1 << 24) - 1;

    #[test]
    fn rejects_solution_above_tx_gas_limit() {
        // Block limit (120M) is high enough that half the block (60M) exceeds
        // the configured per-tx limit (EIP-7825 cap, 16,777,215). The per-tx
        // limit must win.
        let block_limit = gas(120_000_000);
        let tx_gas_limit = gas(EIP_7825_MAINNET_TX_GAS_CAP);
        let estimate = gas(20_000_000);
        let err = Gas::new(estimate, block_limit, tx_gas_limit).unwrap_err();
        match err {
            solution::Error::GasLimitExceeded(used, limit) => {
                assert_eq!(used, estimate);
                assert_eq!(limit, tx_gas_limit);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn accepts_solution_at_tx_gas_limit() {
        let block_limit = gas(120_000_000);
        let tx_gas_limit = gas(EIP_7825_MAINNET_TX_GAS_CAP);
        let result = Gas::new(tx_gas_limit, block_limit, tx_gas_limit).unwrap();
        assert_eq!(result.estimate, tx_gas_limit);
        // The 2x buffer would otherwise push limit to 2 * tx_gas_limit; the
        // min(max_gas, ...) clamp must keep it at the configured cap.
        assert_eq!(result.limit, tx_gas_limit);
    }

    #[test]
    fn small_block_limit_still_caps_at_half() {
        // On chains with a low block gas limit, the half-block cap is tighter
        // than the configured per-tx limit and must keep applying.
        let block_limit = gas(10_000_000);
        let tx_gas_limit = gas(EIP_7825_MAINNET_TX_GAS_CAP);
        let estimate = gas(6_000_000);
        let err = Gas::new(estimate, block_limit, tx_gas_limit).unwrap_err();
        match err {
            solution::Error::GasLimitExceeded(_, limit) => assert_eq!(limit, gas(5_000_000)),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn high_tx_gas_limit_lets_half_block_bind() {
        // Non-Fusaka chain: tx_gas_limit configured well above half the block,
        // so the half-block cap is the binding limit.
        let block_limit = gas(120_000_000);
        let tx_gas_limit = gas(100_000_000);
        let estimate = gas(70_000_000);
        let err = Gas::new(estimate, block_limit, tx_gas_limit).unwrap_err();
        match err {
            solution::Error::GasLimitExceeded(_, limit) => assert_eq!(limit, gas(60_000_000)),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn submission_fee_prefers_override_over_driver_estimate() {
        // No override: driver estimate, doubled.
        assert_eq!(submission_max_fee_per_gas(100, None), U256::from(200));
        // Override above the doubled estimate: use the override.
        assert_eq!(submission_max_fee_per_gas(100, Some(500)), U256::from(500));
        // Override below the doubled estimate: still the override, not driver * 2.
        assert_eq!(submission_max_fee_per_gas(100, Some(50)), U256::from(50));
    }
}
