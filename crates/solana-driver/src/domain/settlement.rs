//! Settlement encoding.

use {
    super::{Order, Side, auction::Id, order_uid::OrderUid, solution::Solution},
    cow_settlement_client::instructions::{
        BeginSettle,
        CreateBuffers,
        FinalizeSettle,
        FinalizedIntent,
        InitializedIntent,
        Pull,
    },
    cow_settlement_interface::{
        data::intent::{OrderIntent, OrderKind},
        pda::{buffer::find_buffer_pda, order::find_order_pda},
    },
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_sdk::{
        hash::Hash,
        instruction::Instruction,
        message::{AddressLookupTableAccount, VersionedMessage, v0::Message as MessageV0},
        pubkey::Pubkey,
        signer::{Signer, keypair::Keypair},
        transaction::VersionedTransaction,
    },
};

/// A prepared settlement and its transaction encoding.
///
/// A `Settlement` holds the orders that a solution fills, the solution, and the
/// facts the encode path needs (compute budget, missing buffers).
///
/// The transaction runs `BeginSettle` (pulls sell tokens into buffer PDAs), the
/// solver interactions, then `FinalizeSettle` (pushes buy tokens out of buffer
/// PDAs).
#[derive(Clone, Debug)]
pub struct Settlement {
    /// The settlement program id.
    program_id: Pubkey,
    auction_id: Id,
    /// The orders this settlement fills.
    orders: Vec<Order>,
    solution: Solution,
    /// The compute-unit limit for the settlement transaction.
    cu_limit: u32,
    /// Token mints whose buffer PDAs do not exist on chain yet, sorted and
    /// deduplicated.
    missing_buffers: Vec<Pubkey>,
}

impl Settlement {
    /// Build a settlement and validate its orders.
    ///
    /// Each wire order PDA must match the PDA derived from its uid, and each
    /// order must be filled within the same constraints as the EVM settlement
    /// contract.
    pub fn new(
        program_id: Pubkey,
        auction_id: Id,
        mut orders: Vec<Order>,
        solution: Solution,
        cu_limit: u32,
        mut missing_buffers: Vec<Pubkey>,
    ) -> Result<Self, Error> {
        validate_orders(&program_id, &mut orders, &solution)?;
        // Sort and dedup the missing buffers.
        missing_buffers.sort_unstable();
        missing_buffers.dedup();
        Ok(Self {
            program_id,
            auction_id,
            orders,
            solution,
            cu_limit,
            missing_buffers,
        })
    }

    /// Builds the settlement instruction list
    pub fn instructions(&self, payer: Pubkey) -> Result<Vec<Instruction>, Error> {
        // Prepare each order for settlement: resolve its executed amounts and build its
        // intent, sell-mint pull, and buy-mint push.
        let settlement_orders: Vec<SettlementOrder> = self
            .orders
            .iter()
            .map(|order| {
                let amounts = executed_amounts(order, &self.solution)?;
                Ok(SettlementOrder::new(order, &self.program_id, amounts))
            })
            .collect::<Result<_, Error>>()?;

        // Build the BeginSettle and FinalizeSettle inputs.
        let (initialized_intents, finalized_intents): (Vec<_>, Vec<_>) = settlement_orders
            .iter()
            .map(|data| {
                (
                    InitializedIntent {
                        intent: &data.intent,
                        pulls: data.pulls.as_slice(),
                    },
                    FinalizedIntent {
                        intent: &data.intent,
                        mint: data.buy_mint,
                        amount: data.buy_amount,
                    },
                )
            })
            .unzip();

        // Start populating the instruction list.
        let mut instructions = Vec::new();

        // Set the compute limit TODO: Once we have CU price estimation, add the
        // respective `ComputeBudget::set_compute_unit_price` instruction too.
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            self.cu_limit,
        ));
        // Insert `CreateBuffers instructions in case there are missing Buffer accounts.
        if !self.missing_buffers.is_empty() {
            instructions.push(
                CreateBuffers {
                    program_id: self.program_id,
                    payer,
                    mints: &self.missing_buffers,
                }
                .into(),
            );
        }

        // BeginSettle and FinalizeSettle reference each other by index, so compute
        // their positions before pushing them.
        let begin_ix_index =
            u16::try_from(instructions.len()).map_err(|_| Error::InstructionIndexOverflow)?;
        let finalize_ix_index =
            u16::try_from(instructions.len() + 1 + self.solution.interactions.len())
                .map_err(|_| Error::InstructionIndexOverflow)?;

        instructions.push(
            BeginSettle {
                program_id: self.program_id,
                finalize_ix_index,
                auction_id: self.auction_id.get(),
                orders: &initialized_intents,
            }
            .into(),
        );
        instructions.extend(self.solution.interactions.iter().cloned());
        instructions.push(
            FinalizeSettle {
                program_id: self.program_id,
                begin_ix_index,
                orders: &finalized_intents,
            }
            .into(),
        );

        Ok(instructions)
    }

    /// Encode the settlement as a signed v0 transaction.
    pub fn encode(
        &self,
        signer: &Keypair,
        blockhash: Hash,
        lookup_tables: &[AddressLookupTableAccount],
    ) -> Result<VersionedTransaction, Error> {
        let instructions = self.instructions(signer.pubkey())?;
        let message =
            MessageV0::try_compile(&signer.pubkey(), &instructions, lookup_tables, blockhash)?;
        let transaction = VersionedTransaction::try_new(VersionedMessage::V0(message), &[signer])?;
        Ok(transaction)
    }
}

impl From<&Order> for OrderIntent {
    fn from(order: &Order) -> Self {
        OrderIntent {
            owner: order.owner,
            buy_token_account: order.buy_token_account,
            sell_token_account: order.sell_token_account,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            valid_to: order.valid_to,
            kind: match order.side {
                Side::Sell => OrderKind::Sell,
                Side::Buy => OrderKind::Buy,
            },
            partially_fillable: order.partially_fillable,
            // TODO: `app_data` is a placeholder until the `autopilot-svm` sends it.
            app_data: [0; 32],
        }
    }
}

/// The executed sell and buy amounts for one order.
struct ExecutedAmounts {
    sell: u64,
    buy: u64,
}

/// Validate the settlement's orders: reject mismatched PDAs, dedup, and check
/// each order against the solution.
fn validate_orders(
    program_id: &Pubkey,
    orders: &mut Vec<Order>,
    solution: &Solution,
) -> Result<(), Error> {
    // Dedup the orders.
    orders.sort_unstable();
    orders.dedup();

    // Reject trades whose order uid matches no order in the settlement.
    for trade in solution.trades.iter() {
        if !orders.iter().any(|order| order.uid == trade.order_uid) {
            return Err(Error::NoOrderForTrade(trade.order_uid));
        }
    }

    // Validate each order against the solution.
    for order in orders.iter() {
        // Reject orders whose wire PDA does not match the derived PDA.
        // TODO: Ideally we'd also verify that the uid is the hash of the order's
        // intent (intent → uid), not just that the PDA derives from the uid.
        let (derived_pda, _) = find_order_pda(program_id, &Hash::new_from_array(order.uid.0));
        if derived_pda != order.order_pda {
            return Err(Error::OrderPdaMismatch(
                order.order_pda,
                derived_pda,
                order.uid,
            ));
        }

        let amounts = executed_amounts(order, solution)?;
        let (filled, target) = match order.side {
            Side::Sell => (amounts.sell, order.sell_amount),
            Side::Buy => (amounts.buy, order.buy_amount),
        };

        // A non-partially-fillable order must be filled exactly.
        if !order.partially_fillable && filled != target {
            return Err(Error::NotExactlyFilled(order.uid));
        }
        // No order may be filled for more than its target.
        if filled > target {
            return Err(Error::Overfill(order.uid));
        }
        // The executed price must not be worse than the order's limit price.
        if u128::from(amounts.buy) * u128::from(order.sell_amount)
            < u128::from(amounts.sell) * u128::from(order.buy_amount)
        {
            return Err(Error::LimitPriceViolated(order.uid));
        }
    }
    Ok(())
}

/// The total executed amounts for one order, summed across the trades that fill
/// it.
fn executed_amounts(order: &Order, solution: &Solution) -> Result<ExecutedAmounts, Error> {
    let mut trades = solution.trades.iter().filter(|t| t.order_uid == order.uid);

    let first = trades.next().ok_or(Error::NoTradeForOrder(order.uid))?;

    trades
        .try_fold(
            ExecutedAmounts {
                sell: first.executed_sell,
                buy: first.executed_buy,
            },
            |amounts, trade| {
                Some(ExecutedAmounts {
                    sell: amounts.sell.checked_add(trade.executed_sell)?,
                    buy: amounts.buy.checked_add(trade.executed_buy)?,
                })
            },
        )
        .ok_or(Error::ExecutedAmountOverflow)
}

/// An order as prepared for the settlement transaction.
struct SettlementOrder {
    intent: OrderIntent,
    pulls: Vec<Pull>,
    buy_mint: Pubkey,
    buy_amount: u64,
}

impl SettlementOrder {
    /// Build a settlement order from a domain order: its intent, its sell-mint
    /// pull, and its buy-mint push.
    fn new(order: &Order, program_id: &Pubkey, amounts: ExecutedAmounts) -> Self {
        Self {
            intent: order.into(),
            pulls: vec![Pull {
                destination: find_buffer_pda(program_id, &order.sell_token).0,
                amount: amounts.sell,
            }],
            buy_mint: order.buy_token,
            buy_amount: amounts.buy,
        }
    }
}

/// An error from the settlement encoding.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// No trade fills the given order.
    #[error("no trade fills order {0}")]
    NoTradeForOrder(OrderUid),
    /// No order matches the given trade.
    #[error("no order matches trade {0}")]
    NoOrderForTrade(OrderUid),
    /// The sum of the executed amounts overflowed `u64`.
    #[error("executed amounts overflow u64")]
    ExecutedAmountOverflow,
    /// The order is not partially fillable but was not filled exactly.
    #[error("order {0} was not filled exactly")]
    NotExactlyFilled(OrderUid),
    /// The order was filled for more than its target amount.
    #[error("order {0} was overfilled")]
    Overfill(OrderUid),
    /// The order's limit price was violated.
    #[error("order {0} violated its limit price")]
    LimitPriceViolated(OrderUid),
    /// The wire-provided order PDA does not match the derived PDA.
    #[error("order PDA {0} does not match the derived PDA {1} for uid {2}")]
    OrderPdaMismatch(Pubkey, Pubkey, OrderUid),
    /// The transaction failed to compile.
    #[error("failed to compile transaction: {0}")]
    Compile(#[from] solana_sdk::message::CompileError),
    /// The transaction failed to sign.
    #[error("failed to sign transaction: {0}")]
    Sign(#[from] solana_sdk::signer::SignerError),
    /// The instruction index does not fit in `u16`.
    #[error("instruction index does not fit in u16")]
    InstructionIndexOverflow,
}

#[cfg(test)]
mod tests {
    // TODO: The fixtures set `uid` to an arbitrary constant and derive the PDA from
    // it, so the uid → PDA relationship holds. In prod, the uid is the hash of the
    // order's intent (intent → uid → PDA), which the fixtures here don't model yet.
    // Currently, the settlement validation logic only checks uid → PDA, so this is
    // fine for now.
    use {
        super::*,
        crate::domain::Trade,
        cow_settlement_interface::instruction::{
            InstructionInputParsing,
            settle::{BeginSettleInput, FinalizeSettleInput},
        },
        solana_sdk::signer::keypair::Keypair,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn single_trade() -> Trade {
        Trade {
            order_uid: OrderUid([0x11; 32]),
            executed_sell: 1_000,
            executed_buy: 2_000,
        }
    }

    fn solution(trades: Vec<Trade>) -> Solution {
        Solution {
            id: 0,
            solver: pubkey(0x99),
            prices: std::collections::HashMap::from([
                (pubkey(0x33), std::num::NonZero::new(2_000).unwrap()),
                (pubkey(0x44), std::num::NonZero::new(1_000).unwrap()),
            ]),
            trades,
            interactions: Vec::new(),
            address_lookup_tables: Vec::new(),
            cu_estimate: None,
        }
    }

    /// A single-order settlement with no interactions and no setup
    /// instructions. The instruction list is `[SetComputeUnitLimit,
    /// BeginSettle, FinalizeSettle]`.
    fn settlement(program_id: &Pubkey) -> Settlement {
        let order = test_order(program_id);
        Settlement::new(
            *program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![single_trade()]),
            200_000,
            Vec::new(),
        )
        .unwrap()
    }

    /// A default sell order. The order PDA comes from the uid.
    fn test_order(program_id: &Pubkey) -> Order {
        let uid = OrderUid([0x11; 32]);
        let order_pda = find_order_pda(program_id, &Hash::new_from_array(uid.0)).0;
        Order {
            uid,
            owner: pubkey(0x22),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
            sell_token_account: pubkey(0x55),
            buy_token_account: pubkey(0x66),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: 42,
            side: super::super::Side::Sell,
            partially_fillable: false,
            order_pda,
        }
    }

    #[test]
    fn rejects_a_mismatched_order_pda() {
        let program_id = pubkey(0xaa);
        let mut order = test_order(&program_id);
        order.order_pda = pubkey(0xff);
        let derived_pda = find_order_pda(&program_id, &Hash::new_from_array(order.uid.0)).0;

        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![single_trade()]),
            200_000,
            Vec::new(),
        )
        .expect_err("a mismatched order PDA must be rejected");
        assert_eq!(
            err,
            Error::OrderPdaMismatch(pubkey(0xff), derived_pda, OrderUid([0x11; 32]))
        );
    }

    /// Duplicate orders (same uid) are deduped, keeping the first occurrence.
    #[test]
    fn duplicate_orders_are_deduped() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order.clone(), order],
            solution(vec![single_trade()]),
            200_000,
            Vec::new(),
        )
        .unwrap();

        let instructions = settlement.instructions(payer).unwrap();
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.orders.iter().count(), 1);
    }

    #[test]
    fn rejects_a_solution_with_no_trades() {
        let program_id = pubkey(0xaa);
        let order = test_order(&program_id);
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(Vec::new()),
            200_000,
            Vec::new(),
        )
        .expect_err("a solution with no trades must be rejected");
        assert_eq!(err, Error::NoTradeForOrder(OrderUid([0x11; 32])));
    }

    /// A trade whose order uid matches no order in the settlement is rejected.
    #[test]
    fn rejects_a_trade_with_no_matching_order() {
        let program_id = pubkey(0xaa);
        let order = test_order(&program_id);
        let expected = OrderUid([0x22; 32]);
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: expected,
                executed_sell: 1_000,
                executed_buy: 2_000,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("a trade with no matching order must be rejected");
        assert_eq!(err, Error::NoOrderForTrade(expected));
    }

    /// A non-partially-fillable order filled for less than its target is
    /// rejected.
    #[test]
    fn rejects_a_non_partially_fillable_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but only 500 sold / 1_000 bought.
        let order = test_order(&program_id);
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 500,
                executed_buy: 1_000,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("a non-partially-fillable order filled below target must be rejected");
        assert_eq!(err, Error::NotExactlyFilled(OrderUid([0x11; 32])));
    }

    /// A non-partially-fillable buy order filled for less than its target is
    /// rejected. For buy orders the fill target is the buy amount.
    #[test]
    fn rejects_a_non_partially_fillable_buy_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but only 1_000 bought.
        let mut order = test_order(&program_id);
        order.side = Side::Buy;
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 500,
                executed_buy: 1_000,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("a non-partially-fillable buy order filled below target must be rejected");
        assert_eq!(err, Error::NotExactlyFilled(OrderUid([0x11; 32])));
    }

    /// A partially-fillable order filled for less than its target is accepted.
    #[test]
    fn accepts_a_partially_fillable_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let mut order = test_order(&program_id);
        order.partially_fillable = true;
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 500,
                executed_buy: 1_000,
            }]),
            200_000,
            Vec::new(),
        )
        .unwrap();

        settlement.instructions(payer).unwrap();
    }

    /// An order filled for more than its target is rejected.
    #[test]
    fn rejects_an_overfilled_order() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but 1_200 sold / 2_400 bought.
        let mut order = test_order(&program_id);
        order.partially_fillable = true;
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 1_200,
                executed_buy: 2_400,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("an overfilled order must be rejected");
        assert_eq!(err, Error::Overfill(OrderUid([0x11; 32])));
    }

    /// A buy order filled for more than its target is rejected. For buy orders
    /// the fill target is the buy amount.
    #[test]
    fn rejects_an_overfilled_buy_order() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but 2_400 bought.
        let mut order = test_order(&program_id);
        order.side = Side::Buy;
        order.partially_fillable = true;
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 1_200,
                executed_buy: 2_400,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("an overfilled buy order must be rejected");
        assert_eq!(err, Error::Overfill(OrderUid([0x11; 32])));
    }

    /// An order whose executed price is worse than its limit price is rejected.
    #[test]
    fn rejects_an_order_that_violates_its_limit_price() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000. Executed: 1_000 sold / 1_500
        // bought. 1_500 * 1_000 < 1_000 * 2_000, so the limit price is violated.
        let order = test_order(&program_id);
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 1_000,
                executed_buy: 1_500,
            }]),
            200_000,
            Vec::new(),
        )
        .expect_err("an order that violates its limit price must be rejected");
        assert_eq!(err, Error::LimitPriceViolated(OrderUid([0x11; 32])));
    }

    /// More than one trade for the same order: the pull is the total
    /// `executed_sell` and the push is the total `executed_buy`.
    #[test]
    fn multiple_trades_for_the_same_order_are_summed() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        // The fixture order has `sell_amount: 1_000`, `buy_amount: 2_000`. Split it
        // across two trades: 400/800 and 600/1200.
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![
                Trade {
                    order_uid: OrderUid([0x11; 32]),
                    executed_sell: 400,
                    executed_buy: 800,
                },
                Trade {
                    order_uid: OrderUid([0x11; 32]),
                    executed_sell: 600,
                    executed_buy: 1_200,
                },
            ]),
            200_000,
            Vec::new(),
        )
        .unwrap();

        let instructions = settlement.instructions(payer).unwrap();
        // [SetComputeUnitLimit, BeginSettle, FinalizeSettle].
        let begin = &instructions[1];
        let finalize = &instructions[2];

        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        // The pull is the sum: 400 + 600 = 1_000.
        let order = begin_input.orders.iter().next().unwrap();
        assert_eq!(u64::from_le_bytes(order.amounts[0]), 1_000);

        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        // The push is the sum: 800 + 1_200 = 2_000.
        let push = finalize_input.pushes.iter().next().unwrap();
        assert_eq!(push.amount, 2_000);
    }

    /// Two orders in one settlement. Each order gets its own pull and push. We
    /// sum the amounts across the trades for that order.
    #[test]
    fn multiple_orders_in_one_settlement() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);

        let order_a = test_order(&program_id);
        let order_b = Order {
            uid: OrderUid([0x22; 32]),
            owner: pubkey(0x22),
            sell_token: pubkey(0x45),
            buy_token: pubkey(0x46),
            sell_token_account: pubkey(0x67),
            buy_token_account: pubkey(0x68),
            sell_amount: 500,
            buy_amount: 1_000,
            valid_to: 42,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: find_order_pda(&program_id, &Hash::new_from_array([0x22; 32])).0,
        };
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order_a, order_b],
            Solution {
                id: 0,
                solver: pubkey(0x99),
                prices: std::collections::HashMap::from([
                    (pubkey(0x33), std::num::NonZero::new(2_000).unwrap()),
                    (pubkey(0x44), std::num::NonZero::new(1_000).unwrap()),
                ]),
                trades: vec![
                    // Order A: two trades. Sell: 400 + 600 = 1000. Buy: 800 + 1200 = 2000.
                    Trade {
                        order_uid: OrderUid([0x11; 32]),
                        executed_sell: 400,
                        executed_buy: 800,
                    },
                    Trade {
                        order_uid: OrderUid([0x11; 32]),
                        executed_sell: 600,
                        executed_buy: 1_200,
                    },
                    // Order B: one trade. Sell: 500. Buy: 1000.
                    Trade {
                        order_uid: OrderUid([0x22; 32]),
                        executed_sell: 500,
                        executed_buy: 1_000,
                    },
                ],
                interactions: Vec::new(),
                address_lookup_tables: Vec::new(),
                cu_estimate: None,
            },
            200_000,
            Vec::new(),
        )
        .unwrap();

        let instructions = settlement.instructions(payer).unwrap();
        let begin = &instructions[1];
        let finalize = &instructions[2];

        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        // With placeholder `app_data`, the derived PDAs differ from the wire PDAs, so
        // assert pull amounts rather than PDA equality.
        let settled: Vec<u64> = begin_input
            .orders
            .iter()
            .map(|o| u64::from_le_bytes(o.amounts[0]))
            .collect();
        assert_eq!(settled.len(), 2);
        assert!(settled.contains(&1_000));
        assert!(settled.contains(&500));

        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        let pushes: Vec<u64> = finalize_input.pushes.iter().map(|p| p.amount).collect();
        assert_eq!(pushes.len(), 2);
        assert!(pushes.contains(&2_000));
        assert!(pushes.contains(&1_000));
    }

    #[test]
    fn setup_instructions_shift_the_reciprocal_indices() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        // Both the sell-mint and buy-mint buffers are missing.
        let missing_buffers = vec![order.sell_token, order.buy_token];
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![single_trade()]),
            200_000,
            missing_buffers,
        )
        .unwrap();

        let instructions = settlement.instructions(payer).unwrap();

        // [SetComputeUnitLimit, CreateBuffers, BeginSettle, FinalizeSettle].
        assert_eq!(instructions.len(), 4);
        let begin = &instructions[2];
        let finalize = &instructions[3];

        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.finalize_ix_index, 3);

        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        assert_eq!(finalize_input.begin_ix_index, 2);
    }

    #[test]
    fn encode_produces_a_signed_v0_transaction() {
        let program_id = pubkey(0xaa);
        let signer = Keypair::new();
        let settlement = settlement(&program_id);

        let tx = settlement
            .encode(&signer, Hash::new_from_array([0xbb; 32]), &[])
            .unwrap();

        assert!(tx.verify_with_results().iter().all(|r| *r));
        match tx.message {
            VersionedMessage::V0(ref msg) => {
                assert_eq!(msg.recent_blockhash, Hash::new_from_array([0xbb; 32]));
                assert_eq!(msg.instructions.len(), 3);
            }
            VersionedMessage::Legacy(_) | VersionedMessage::V1(_) => panic!("expected v0 message"),
        }
    }
}
