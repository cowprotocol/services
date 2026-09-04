//! Settlement encoding.

use {
    super::{Order, Side, auction::Id, order_uid::OrderUid, solution::Solution},
    crate::infra::blockchain::{
        AccountsSnapshot,
        InvalidAddressLookupTableReason,
        Solana,
        TokenAccountState,
        associated_token_address,
        create_associated_token_account_idempotent,
    },
    cow_settlement_client::instructions::{
        BeginSettle,
        CreateBuffers,
        FinalizeSettle,
        FinalizedIntent,
        InitializedIntent,
        Pull,
    },
    cow_settlement_interface::{
        data::intent::{Flags, OrderIntent, OrderKind},
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

/// A validated settlement.
///
/// A `Settlement` holds the domain facts of a settlement: the orders a
/// solution fills and the solution itself.
///
/// [`Settlement::resolve_accounts`] fetches the chain facts the encoder needs
/// and yields a [`ResolvedSettlement`].
#[derive(Clone, Debug)]
pub struct Settlement {
    /// The settlement program id.
    program_id: Pubkey,
    auction_id: Id,
    /// The orders this settlement fills.
    orders: Vec<Order>,
    solution: Solution,
}

/// A settlement with its on-chain accounts resolved.
///
/// The chain facts (lookup tables, missing setup accounts) are fetched in
/// [`Settlement::resolve_accounts`].
///
/// The transaction optionally sets a compute-unit limit and creates the
/// missing setup accounts (buy-mint buffer PDAs, the payer's sell-mint ATAs),
/// then runs `BeginSettle` (pulls sell tokens into the payer's sell ATAs),
/// the solver interactions, and `FinalizeSettle` (pushes buy tokens out of
/// the buy-mint buffer PDAs).
pub(crate) struct ResolvedSettlement {
    settlement: Settlement,
    /// The solution's resolved address lookup tables.
    lookup_tables: Vec<AddressLookupTableAccount>,
    /// Token mints whose buffer PDAs do not exist on chain yet, sorted and
    /// deduplicated.
    missing_buffers: Vec<Pubkey>,
    /// Sell-token mints for which the payer's ATA does not exist on chain
    /// yet, sorted and deduplicated.
    missing_payer_atas: Vec<Pubkey>,
}

impl Settlement {
    /// Build a settlement and validate its orders.
    ///
    /// Each wire order PDA must match the PDA derived from its uid, and each
    /// order must be filled within the same constraints as the EVM settlement
    /// contract. Orders that already expired are rejected.
    pub fn new(
        program_id: Pubkey,
        auction_id: Id,
        mut orders: Vec<Order>,
        solution: Solution,
    ) -> Result<Self, Error> {
        dedup_orders(&mut orders);
        validate_orders(&program_id, &orders, &solution)?;
        Ok(Self {
            program_id,
            auction_id,
            orders,
            solution,
        })
    }

    /// Resolve the on-chain accounts this settlement needs. The settlement
    /// must create the missing setup accounts before `BeginSettle`.
    pub(crate) async fn resolve_accounts(
        self,
        blockchain: &Solana,
        payer: Pubkey,
    ) -> Result<ResolvedSettlement, ResolveError> {
        let mut buffers = Vec::with_capacity(self.orders.len());
        let mut sell_atas = Vec::with_capacity(self.orders.len());
        for order in &self.orders {
            buffers.push(SetupAccount::new_buffer(order.buy_token, self.program_id));
            sell_atas.push(SetupAccount::new_ata(order.sell_token, payer));
        }

        let addresses = self
            .solution
            .address_lookup_tables
            .iter()
            .copied()
            .chain(buffers.iter().map(|token| token.address))
            .chain(sell_atas.iter().map(|token| token.address));

        let snapshot = blockchain
            .accounts_snapshot(addresses)
            .await
            .map_err(ResolveError::Rpc)?;

        let lookup_tables = self
            .solution
            .address_lookup_tables
            .iter()
            .map(|key| {
                snapshot
                    .lookup_table(key)
                    .map_err(|reason| ResolveError::InvalidAddressLookupTable { key: *key, reason })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut missing_buffers = missing_setup_accounts(&buffers, &snapshot)?;
        missing_buffers.sort_unstable();
        missing_buffers.dedup();
        let mut missing_payer_atas = missing_setup_accounts(&sell_atas, &snapshot)?;
        missing_payer_atas.sort_unstable();
        missing_payer_atas.dedup();

        Ok(ResolvedSettlement {
            settlement: self,
            lookup_tables,
            missing_buffers,
            missing_payer_atas,
        })
    }
}

impl ResolvedSettlement {
    /// Build the settlement instruction list.
    fn instructions(&self, payer: Pubkey) -> Result<Vec<Instruction>, Error> {
        // Prepare each order for settlement: resolve its executed amounts and
        // build its intent, sell-mint pull, and buy-mint push.
        let settlement_orders: Vec<SettlementOrder> = self
            .settlement
            .orders
            .iter()
            .map(|order| {
                let amounts = executed_amounts(order, &self.settlement.solution)?;
                Ok(SettlementOrder::new(order, &payer, amounts))
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
                        amount: data.buy_amount,
                    },
                )
            })
            .unzip();

        // Start populating the instruction list.
        let mut instructions = Vec::new();

        // Set the compute limit. The solver provides an optional CU estimate;
        // if it is missing we fall back to the Solana default. TODO:
        // Once we have CU price estimation, add the respective
        // `ComputeBudget::set_compute_unit_price` instruction too.
        if let Some(cu_limit) = self.settlement.solution.cu_estimate {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(cu_limit));
        }
        // Insert a `CreateBuffers` instruction when buffer accounts are
        // missing.
        if !self.missing_buffers.is_empty() {
            instructions.push(
                CreateBuffers {
                    program_id: self.settlement.program_id,
                    payer,
                    mints: &self.missing_buffers,
                }
                .into(),
            );
        }
        // Create the payer's missing sell-mint ATAs. `BeginSettle` pulls the
        // sell tokens into them, and an SPL transfer into an
        // uninitialized account causes the transaction to revert, so
        // they must exist before `BeginSettle` runs.
        for mint in &self.missing_payer_atas {
            instructions.push(create_associated_token_account_idempotent(
                &payer, &payer, mint,
            ));
        }

        // BeginSettle and FinalizeSettle reference each other by index, so
        // compute their positions before pushing them.
        let begin_ix_index =
            u16::try_from(instructions.len()).map_err(|_| Error::InstructionIndexOverflow)?;
        let finalize_ix_index =
            u16::try_from(instructions.len() + 1 + self.settlement.solution.interactions.len())
                .map_err(|_| Error::InstructionIndexOverflow)?;

        instructions.push(
            BeginSettle {
                program_id: self.settlement.program_id,
                solver: payer,
                finalize_ix_index,
                auction_id: self.settlement.auction_id.get(),
                orders: &initialized_intents,
            }
            .into(),
        );
        instructions.extend(self.settlement.solution.interactions.iter().cloned());
        instructions.push(
            FinalizeSettle {
                program_id: self.settlement.program_id,
                begin_ix_index,
                orders: &finalized_intents,
            }
            .into(),
        );

        Ok(instructions)
    }

    /// Encode the resolved settlement as a signed v0 transaction.
    pub fn encode(self, signer: &Keypair, blockhash: Hash) -> Result<VersionedTransaction, Error> {
        let instructions = self.instructions(signer.pubkey())?;
        let message = MessageV0::try_compile(
            &signer.pubkey(),
            &instructions,
            &self.lookup_tables,
            blockhash,
        )?;
        let transaction = VersionedTransaction::try_new(VersionedMessage::V0(message), &[signer])?;
        Ok(transaction)
    }
}

/// Represents a token account required either as a buffer account or a Solver
/// ATA.
#[derive(Debug, Clone, Copy)]
struct SetupAccount {
    mint: Pubkey,
    address: Pubkey,
}

impl SetupAccount {
    /// Returns the Buffer PDA for the given mint.
    fn new_buffer(mint: Pubkey, program_id: Pubkey) -> Self {
        let address = find_buffer_pda(&program_id, &mint).0;
        Self { mint, address }
    }

    /// Returns a Solver's associated token account for the given mint.
    fn new_ata(mint: Pubkey, owner: Pubkey) -> Self {
        let address = associated_token_address(&owner, &mint);
        Self { mint, address }
    }
}

/// Collect the mints whose setup accounts must be created before
/// `BeginSettle`.
///
/// Usable accounts are filtered out. An account in a state the settlement can
/// neither use nor create over is an invariant violation that would fail on
/// chain, so reject it before submission.
fn missing_setup_accounts(
    tokens: &[SetupAccount],
    snapshot: &AccountsSnapshot,
) -> Result<Vec<Pubkey>, ResolveError> {
    let mut missing = Vec::new();
    for token in tokens {
        match snapshot.token_account_state(&token.address) {
            TokenAccountState::NeedsCreation => missing.push(token.mint),
            TokenAccountState::Initialized => (),
            TokenAccountState::Unexpected { owner, data_len } => {
                tracing::warn!(
                    mint = %token.mint,
                    account = %token.address,
                    %owner,
                    data_len,
                    "setup account is in an unexpected state"
                );
                return Err(ResolveError::UnexpectedSetupAccount {
                    account: token.address,
                    mint: token.mint,
                    owner,
                });
            }
        }
    }
    Ok(missing)
}

/// An error from resolving a settlement's on-chain accounts.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ResolveError {
    /// The batched account fetch failed; nothing was submitted.
    #[error("rpc request failed: {0}")]
    Rpc(#[source] cow_solana_rpc::Error),
    /// An address lookup table referenced by the solution is missing, invalid,
    /// or deactivated.
    #[error("invalid address lookup table {key}: {reason}")]
    InvalidAddressLookupTable {
        key: Pubkey,
        reason: InvalidAddressLookupTableReason,
    },
    /// A setup account (buy-mint buffer PDA or payer sell-mint ATA) exists on
    /// chain in a state the settlement can neither use nor create over.
    #[error("setup account {account} for mint {mint} has unexpected owner {owner}")]
    UnexpectedSetupAccount {
        account: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
    },
}

impl From<&Order> for OrderIntent {
    fn from(order: &Order) -> Self {
        OrderIntent {
            owner: order.owner,
            buy_token_account: order.buy_token_account,
            buy_mint: order.buy_token,
            sell_token_account: order.sell_token_account,
            sell_mint: order.sell_token,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            valid_to: order.valid_to,
            // Off-chain, Ed25519-signed orders only. On-chain-created orders
            // would carry this from the wire.
            flags: Flags {
                created_on_chain: false,
                kind: match order.side {
                    Side::Sell => OrderKind::Sell,
                    Side::Buy => OrderKind::Buy,
                },
                partially_fillable: order.partially_fillable,
            },
            app_data: order.app_data,
        }
    }
}

/// The executed sell and buy amounts for one order.
struct ExecutedAmounts {
    sell: u64,
    buy: u64,
}

fn dedup_orders(orders: &mut Vec<Order>) {
    orders.sort_unstable();
    orders.dedup();
}

/// Reject mismatched PDAs, expired orders, and fills that violate the
/// solution's clearing prices.
fn validate_orders(
    program_id: &Pubkey,
    orders: &[Order],
    solution: &Solution,
) -> Result<(), Error> {
    let now = chrono::Utc::now().timestamp();

    // Reject trades whose order uid matches no order in the settlement.
    for trade in solution.trades.iter() {
        if !orders.iter().any(|order| order.uid == trade.order_uid) {
            return Err(Error::NoOrderForTrade(trade.order_uid));
        }
    }

    // Validate each order against the solution.
    for order in orders.iter() {
        // Reject expired orders: the program rejects them on chain with
        // `OrderExpired` (an order is valid while `now <= valid_to`), so
        // submitting one would only pay fees for a guaranteed revert.
        if i64::from(order.valid_to) < now {
            return Err(Error::OrderExpired(order.uid));
        }

        // Reject orders whose uid is not the hash of their reconstructed
        // intent. This closes the intent → uid → PDA chain: the wire
        // `order_pda` is only trusted once it derives from a uid that
        // itself matches the intent.
        let intent_uid = OrderIntent::from(order).uid();
        if intent_uid != Hash::new_from_array(order.uid.0) {
            return Err(Error::OrderIntentMismatch(intent_uid, order.uid));
        }

        // Reject orders whose wire PDA does not match the PDA derived from the
        // (now intent-consistent) uid.
        let (derived_pda, _) = find_order_pda(program_id, &intent_uid);
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
        //
        // Note: the fill caps compare against each order's *full* amounts, not
        // its remaining amounts. The driver does not read the order
        // PDA's fill state (`amount_withdrawn`/`amount_received`), so
        // it cannot know how much prior settlements consumed.
        //
        // Prior fills only shrink the remaining amount, so the full amount is a
        // hard upper bound. This check therefore never rejects a
        // settlement that could succeed on chain. But a fill over the
        // *remaining* amount and within the full amount passes here and
        // fails on chain with `FillExceedsOrderAmount`. The program's
        // cumulative check is the authority. This check exists only to
        // avoid paying fees for transactions that are guaranteed to fail.
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
    buy_amount: u64,
}

impl SettlementOrder {
    /// Build a settlement order from a domain order: its intent, its sell-mint
    /// pull into the payer's sell ATA, and its buy-mint push.
    ///
    /// The swap output lands in the buy-mint buffer PDA (see
    /// `infra/solver/dto/auction.rs`), so the sell tokens are pulled into the
    /// payer's sell ATA rather than a buffer.
    fn new(order: &Order, payer: &Pubkey, amounts: ExecutedAmounts) -> Self {
        Self {
            intent: order.into(),
            pulls: vec![Pull {
                destination: associated_token_address(payer, &order.sell_token),
                amount: amounts.sell,
            }],
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
    /// The order has already expired.
    #[error("order {0} expired")]
    OrderExpired(OrderUid),
    /// The wire-provided order PDA does not match the derived PDA.
    #[error("order PDA {0} does not match the derived PDA {1} for uid {2}")]
    OrderPdaMismatch(Pubkey, Pubkey, OrderUid),
    /// The uid is not the hash of the order's reconstructed intent.
    #[error("order uid {1} does not match the hash of its intent {0}")]
    OrderIntentMismatch(Hash, OrderUid),
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
    use {
        super::*,
        crate::domain::Trade,
        cow_settlement_interface::instruction::{
            InstructionInputParsing,
            settle::{BeginSettleInput, FinalizeSettleInput},
        },
        std::slice,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// A default sell order with `customize` applied before the uid and order
    /// PDA are derived.
    fn test_order_with(program_id: &Pubkey, customize: impl FnOnce(&mut Order)) -> Order {
        let mut order = Order {
            uid: OrderUid([0; 32]), // re-derived below
            owner: pubkey(0x22),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
            sell_token_account: pubkey(0x55),
            buy_token_account: pubkey(0x66),
            sell_amount: 1_000,
            buy_amount: 2_000,
            // Far future so the expiry validation passes.
            valid_to: u32::MAX,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: Pubkey::default(), // re-derived below
            app_data: [0x77; 32],
        };
        customize(&mut order);
        let uid = OrderIntent::from(&order).uid();
        order.uid = OrderUid(uid.to_bytes());
        order.order_pda = find_order_pda(program_id, &uid).0;
        order
    }

    /// A default sell order.
    fn test_order(program_id: &Pubkey) -> Order {
        test_order_with(program_id, |_| ())
    }

    /// A trade filling the order with the given uid.
    fn trade(order_uid: OrderUid, executed_sell: u64, executed_buy: u64) -> Trade {
        Trade {
            order_uid,
            executed_sell,
            executed_buy,
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
            cu_estimate: Some(200_000),
        }
    }

    /// Convenience wrapper around `Settlement::new` using the test fixture
    /// defaults.
    fn test_settlement(orders: &[Order], trades: &[Trade]) -> Result<Settlement, Error> {
        Settlement::new(
            pubkey(0xaa),
            Id::new(7).unwrap(),
            orders.to_vec(),
            solution(trades.to_vec()),
        )
    }

    fn resolve_for_test(settlement: Settlement) -> ResolvedSettlement {
        ResolvedSettlement {
            settlement,
            lookup_tables: Vec::new(),
            missing_buffers: Vec::new(),
            missing_payer_atas: Vec::new(),
        }
    }

    #[test]
    fn rejects_a_mismatched_order_pda() {
        let program_id = pubkey(0xaa);
        let mut order = test_order(&program_id);
        let derived_pda = order.order_pda;
        order.order_pda = pubkey(0xff);
        let uid = order.uid;

        let err = test_settlement(&[order], &[trade(uid, 1_000, 2_000)])
            .expect_err("a mismatched order PDA must be rejected");
        assert_eq!(err, Error::OrderPdaMismatch(pubkey(0xff), derived_pda, uid));
    }

    /// An order whose uid is not the hash of its reconstructed intent is
    /// rejected.
    #[test]
    fn rejects_an_order_intent_uid_mismatch() {
        let program_id = pubkey(0xaa);
        let mut order = test_order(&program_id);
        // The trade matches the order's stored (bogus) uid, so the intent
        // integrity check — not the trade look-up — rejects the order.
        let bogus_uid = OrderUid([0xff; 32]);
        order.uid = bogus_uid;
        let intent_uid = OrderIntent::from(&order).uid();

        let err = test_settlement(&[order], &[trade(bogus_uid, 1_000, 2_000)])
            .expect_err("an order whose uid does not match its intent must be rejected");
        assert_eq!(err, Error::OrderIntentMismatch(intent_uid, bogus_uid));
    }

    /// Duplicate orders (same uid) are deduped, keeping the first occurrence.
    #[test]
    fn duplicate_orders_are_deduped() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let settlement = test_settlement(
            &[order.clone(), order.clone()],
            &[trade(order.uid, 1_000, 2_000)],
        )
        .unwrap();

        let instructions = resolve_for_test(settlement).instructions(payer).unwrap();
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.orders.iter().count(), 1);
    }

    /// The sell tokens are pulled into the payer's sell ATA, not a buffer, so
    /// that the solver's swap (whose output lands in the buy-mint buffer) can
    /// spend them directly.
    #[test]
    fn sell_pull_destination_is_the_payer_sell_ata() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let sell_token = order.sell_token;
        let uid = order.uid;
        let settlement = test_settlement(&[order], &[trade(uid, 1_000, 2_000)]).unwrap();

        let instructions = resolve_for_test(settlement).instructions(payer).unwrap();
        // [SetComputeUnitLimit, BeginSettle, FinalizeSettle].
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        let destination = begin_input.orders.iter().next().unwrap().destinations[0];
        assert_eq!(destination, associated_token_address(&payer, &sell_token),);
    }

    /// An order whose `valid_to` has passed is rejected.
    #[test]
    fn rejects_an_expired_order() {
        let program_id = pubkey(0xaa);
        let order = test_order_with(&program_id, |order| order.valid_to = 42);
        let uid = order.uid;
        let err = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(vec![trade(uid, 1_000, 2_000)]),
        )
        .expect_err("an expired order must be rejected");
        assert_eq!(err, Error::OrderExpired(uid));
    }

    #[test]
    fn rejects_a_solution_with_no_trades() {
        let program_id = pubkey(0xaa);
        let order = test_order(&program_id);
        let uid = order.uid;
        let err =
            test_settlement(&[order], &[]).expect_err("a solution with no trades must be rejected");
        assert_eq!(err, Error::NoTradeForOrder(uid));
    }

    /// A trade whose order uid matches no order in the settlement is rejected.
    #[test]
    fn rejects_a_trade_with_no_matching_order() {
        let program_id = pubkey(0xaa);
        let order = test_order(&program_id);
        let stray_uid = OrderUid([0xff; 32]);
        let err = test_settlement(&[order], &[trade(stray_uid, 1_000, 2_000)])
            .expect_err("a trade with no matching order must be rejected");
        assert_eq!(err, Error::NoOrderForTrade(stray_uid));
    }

    /// A non-partially-fillable order filled for less than its target is
    /// rejected.
    #[test]
    fn rejects_a_non_partially_fillable_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but only 500 sold / 1_000
        // bought.
        let order = test_order(&program_id);
        let uid = order.uid;
        let err = test_settlement(&[order], &[trade(uid, 500, 1_000)])
            .expect_err("a non-partially-fillable order filled below target must be rejected");
        assert_eq!(err, Error::NotExactlyFilled(uid));
    }

    /// A non-partially-fillable buy order filled for less than its target is
    /// rejected. For buy orders the fill target is the buy amount.
    #[test]
    fn rejects_a_non_partially_fillable_buy_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but only 1_000 bought.
        let order = test_order_with(&program_id, |order| order.side = Side::Buy);
        let uid = order.uid;
        let err = test_settlement(&[order], &[trade(uid, 500, 1_000)])
            .expect_err("a non-partially-fillable buy order filled below target must be rejected");
        assert_eq!(err, Error::NotExactlyFilled(uid));
    }

    /// A partially-fillable order filled for less than its target is accepted.
    #[test]
    fn accepts_a_partially_fillable_order_filled_below_target() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order_with(&program_id, |order| order.partially_fillable = true);
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 500, 1_000)]).unwrap();

        resolve_for_test(settlement).instructions(payer).unwrap();
    }

    /// An order filled for more than its target is rejected.
    #[test]
    fn rejects_an_overfilled_order() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but 1_200 sold / 2_400 bought.
        let order = test_order_with(&program_id, |order| order.partially_fillable = true);
        let uid = order.uid;
        let err = test_settlement(&[order], &[trade(uid, 1_200, 2_400)])
            .expect_err("an overfilled order must be rejected");
        assert_eq!(err, Error::Overfill(uid));
    }

    /// A buy order filled for more than its target is rejected. For buy orders
    /// the fill target is the buy amount.
    #[test]
    fn rejects_an_overfilled_buy_order() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000, but 2_400 bought.
        let order = test_order_with(&program_id, |order| {
            order.side = Side::Buy;
            order.partially_fillable = true;
        });
        let uid = order.uid;
        let err = test_settlement(&[order], &[trade(uid, 1_200, 2_400)])
            .expect_err("an overfilled buy order must be rejected");
        assert_eq!(err, Error::Overfill(uid));
    }

    /// An order whose executed price is worse than its limit price is rejected.
    #[test]
    fn rejects_an_order_that_violates_its_limit_price() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000. Executed: 1_000 sold / 1_500
        // bought. 1_500 * 1_000 < 1_000 * 2_000, so the limit price is
        // violated.
        let order = test_order(&program_id);
        let uid = order.uid;
        let err = test_settlement(&[order], &[trade(uid, 1_000, 1_500)])
            .expect_err("an order that violates its limit price must be rejected");
        assert_eq!(err, Error::LimitPriceViolated(uid));
    }

    /// More than one trade for the same order: the pull is the total
    /// `executed_sell` and the push is the total `executed_buy`.
    #[test]
    fn multiple_trades_for_the_same_order_are_summed() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let uid = order.uid;
        // The fixture order has `sell_amount: 1_000`, `buy_amount: 2_000`.
        // Split it across two trades: 400/800 and 600/1200.
        let settlement =
            test_settlement(&[order], &[trade(uid, 400, 800), trade(uid, 600, 1_200)]).unwrap();

        let instructions = resolve_for_test(settlement).instructions(payer).unwrap();
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
        // Order B: distinct tokens, accounts, and amounts, so its uid differs.
        let order_b = test_order_with(&program_id, |order| {
            order.sell_token = pubkey(0x45);
            order.buy_token = pubkey(0x46);
            order.sell_token_account = pubkey(0x67);
            order.buy_token_account = pubkey(0x68);
            order.sell_amount = 500;
            order.buy_amount = 1_000;
        });
        let (uid_a, uid_b) = (order_a.uid, order_b.uid);
        let settlement = test_settlement(
            &[order_a, order_b],
            &[
                // Order A: two trades. Sell: 400 + 600 = 1000. Buy: 800 + 1200 = 2000.
                trade(uid_a, 400, 800),
                trade(uid_a, 600, 1_200),
                // Order B: one trade. Sell: 500. Buy: 1000.
                trade(uid_b, 500, 1_000),
            ],
        )
        .unwrap();

        let instructions = resolve_for_test(settlement).instructions(payer).unwrap();
        let begin = &instructions[1];
        let finalize = &instructions[2];

        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        // Assert pull amounts rather than PDA equality; the tests above already
        // cover the PDA derivation cross-check.
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
        let trades = vec![trade(order.uid, 1_000, 2_000)];
        // Two missing buffer PDAs (here both the sell-mint and buy-mint) plus
        // a missing payer ATA shift the reciprocal BeginSettle/FinalizeSettle
        // indices.
        let missing_buffers = vec![order.sell_token, order.buy_token];
        let missing_payer_atas = vec![order.sell_token];
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(trades),
        )
        .unwrap();
        let resolved = ResolvedSettlement {
            settlement,
            lookup_tables: Vec::new(),
            missing_buffers,
            missing_payer_atas,
        };

        let instructions = resolved.instructions(payer).unwrap();

        // [SetComputeUnitLimit, CreateBuffers, CreateAtaIdempotent,
        // BeginSettle, FinalizeSettle].
        assert_eq!(instructions.len(), 5);
        let begin = &instructions[3];
        let finalize = &instructions[4];

        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.finalize_ix_index, 4);

        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        assert_eq!(finalize_input.begin_ix_index, 3);
    }

    /// app_data is a determinant of the order uid. If the code regressed to
    /// the old [0; 32] placeholder, the uid would change and settlement would
    /// reject the order as a PDA mismatch.
    #[test]
    fn app_data_is_determinant_to_order_uid() {
        let program_id = pubkey(0xaa);

        let order_with_app_data = test_order_with(&program_id, |order| {
            order.app_data = [0xde; 32];
        });
        let order_with_placeholder = test_order_with(&program_id, |order| {
            order.app_data = [0; 32];
        });

        assert_ne!(
            order_with_app_data.uid, order_with_placeholder.uid,
            "orders that differ only in app_data must have different uids"
        );
    }

    /// The order uid commits to the sell and buy token accounts. Each token
    /// account has one immutable mint on chain, so the uid also pins the
    /// mints a settlement can touch: two orders with the same uid necessarily
    /// move the same tokens, and the wire mint fields are annotations that
    /// can at worst make the transaction fail on chain, never redirect it.
    #[test]
    fn token_accounts_are_determinant_to_order_uid() {
        let program_id = pubkey(0xaa);
        let base = test_order(&program_id);
        let with_other_sell_account = test_order_with(&program_id, |order| {
            order.sell_token_account = pubkey(0x56);
        });
        let with_other_buy_account = test_order_with(&program_id, |order| {
            order.buy_token_account = pubkey(0x67);
        });

        assert_ne!(
            base.uid, with_other_sell_account.uid,
            "orders that differ only in the sell token account must have different uids"
        );
        assert_ne!(
            base.uid, with_other_buy_account.uid,
            "orders that differ only in the buy token account must have different uids"
        );
    }
}
