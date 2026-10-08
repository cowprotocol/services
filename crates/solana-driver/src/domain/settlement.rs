//! Settlement encoding.

use {
    super::{
        Order,
        Side,
        auction::Id,
        order_uid::OrderUid,
        solution::Solution,
        solver_fee::BPS_DENOMINATOR,
    },
    crate::infra::blockchain::{
        AccountsSnapshot,
        InvalidAddressLookupTableReason,
        InvalidMintReason,
        Solana,
        TokenAccountState,
        associated_token_address,
        close_token_account,
        create_associated_token_account_idempotent,
        require_token_balance,
    },
    cow_settlement_client::instruction::{
        BeginSettle,
        CreateBuffers,
        FinalizeSettle,
        FinalizedIntent,
        InitializedIntent,
        Pull,
    },
    cow_settlement_interface::{
        data::intent::{Asset, Flags, OrderIntent, OrderKind, TokenAsset},
        pda::{buffer::find_buffer_pda, order::find_order_pda, state::STATE_PDA},
        token_program::TokenProgram,
    },
    cow_solana_signer::Signer,
    itertools::Itertools,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_sdk::{
        hash::Hash,
        instruction::Instruction,
        message::{AddressLookupTableAccount, VersionedMessage, v0::Message as MessageV0},
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    solana_system_interface::instruction::transfer,
    spl_token_interface::native_mint,
    std::{
        collections::{HashMap, HashSet},
        num::NonZeroU64,
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

/// A settlement with its on-chain accounts resolved by
/// [`Settlement::resolve_accounts`]: lookup tables and the setup accounts
/// the settlement transaction requires.
///
/// The transaction sets its compute budget (the solution's optional unit limit
/// and the driver's unit price) and creates the missing setup accounts
/// (buy-mint buffer PDAs, the payer's sell-mint ATAs and its wSOL ATA for
/// native SOL buys, the orders' buy-mint ATAs), then runs `BeginSettle` (pulls
/// sell tokens into the payer's sell ATAs), the solver interactions, the
/// funding of the state PDA for native SOL buys, and `FinalizeSettle` (pushes
/// buy tokens out of the buy-mint buffer PDAs and native SOL out of the state
/// PDA).
pub(crate) struct ResolvedSettlement {
    settlement: Settlement,
    /// The fee payer, who also signs `BeginSettle` as the solver.
    payer: Pubkey,
    /// The solution's resolved address lookup tables.
    lookup_tables: Vec<AddressLookupTableAccount>,
    /// Token mints whose buffer PDAs do not exist on chain yet, sorted and
    /// deduplicated.
    missing_buffers: Vec<Pubkey>,
    /// The ATAs the transaction creates: the payer's sell-mint ATAs and the
    /// orders' buy-mint ATAs missing on chain, plus the payer's wSOL ATA
    /// whenever the settlement uses it. Sorted and deduplicated.
    missing_atas: Vec<Ata>,
    token_programs: TokenPrograms,
    /// The largest share of the native SOL payouts that the payer's own SOL
    /// covers when the swap output falls short. `None` covers nothing.
    max_native_shortfall: Option<MaxNativeShortfall>,
}

/// The token program of every mint in [`Settlement::mints`].
struct TokenPrograms(HashMap<Pubkey, TokenProgram>);

impl TokenPrograms {
    /// `mint`'s token program. A miss means `mint` is outside
    /// [`Settlement::mints`].
    fn get(&self, mint: Pubkey) -> Result<TokenProgram, UnresolvedMint> {
        self.0.get(&mint).copied().ok_or(UnresolvedMint(mint))
    }
}

/// A token program lookup for a mint outside [`Settlement::mints`].
#[derive(Debug, PartialEq, thiserror::Error)]
#[error("no token program resolved for mint {0}")]
pub struct UnresolvedMint(pub Pubkey);

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
        // ATA addresses derive from the mint's token program, so the mints are
        // read before the accounts derived from them.
        let token_programs = blockchain
            .token_programs(self.mints())
            .await
            .map_err(ResolveError::Rpc)?
            .into_iter()
            .map(|(mint, program)| {
                program
                    .map(|program| (mint, program))
                    .map_err(|reason| ResolveError::InvalidMint { mint, reason })
            })
            .collect::<Result<_, _>>()
            .map(TokenPrograms)?;
        let (buffers, payer_atas) = self.setup_accounts(payer, &token_programs)?;

        let addresses = self
            .solution
            .address_lookup_tables
            .iter()
            .copied()
            .chain(buffers.iter().map(|token| token.address))
            .chain(payer_atas.iter().map(|token| token.address))
            .chain(token_buys(&self.orders).map(|order| order.buy_token_account));

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

        let (missing_buffers, missing_atas) = accounts_to_create(
            &self.orders,
            &buffers,
            &payer_atas,
            payer,
            &snapshot,
            &token_programs,
        )?;

        Ok(ResolvedSettlement {
            settlement: self,
            payer,
            lookup_tables,
            missing_buffers,
            missing_atas,
            token_programs,
            max_native_shortfall: None,
        })
    }

    /// Every mint the settlement moves: the sell mints, the token buy mints,
    /// and wSOL when an order buys native SOL.
    fn mints(&self) -> HashSet<Pubkey> {
        let wsol = self
            .orders
            .iter()
            .any(Order::buys_native_sol)
            .then_some(native_mint::ID);
        self.orders
            .iter()
            .flat_map(|order| {
                let buy = (!order.buys_native_sol()).then_some(order.buy_token);
                [Some(order.sell_token), buy]
            })
            .flatten()
            .chain(wsol)
            .collect()
    }

    /// The buffer PDAs and payer ATAs the settlement needs: a buffer per
    /// token buy mint, a payer ATA per sell mint, and the payer's wSOL ATA when
    /// an order buys native SOL.
    fn setup_accounts(
        &self,
        payer: Pubkey,
        token_programs: &TokenPrograms,
    ) -> Result<(Vec<SetupAccount>, Vec<SetupAccount>), ResolveError> {
        let buffers = self
            .orders
            .iter()
            .filter(|order| !order.buys_native_sol())
            .map(|order| SetupAccount::new_buffer(order.buy_token, self.program_id))
            .collect();
        let wsol = self
            .orders
            .iter()
            .any(Order::buys_native_sol)
            .then_some(native_mint::ID);
        let payer_atas = self
            .orders
            .iter()
            .map(|order| order.sell_token)
            .chain(wsol)
            .map(|mint| {
                Ok(SetupAccount::new_ata(
                    mint,
                    payer,
                    token_programs.get(mint)?,
                ))
            })
            .collect::<Result<_, ResolveError>>()?;
        Ok((buffers, payer_atas))
    }
}

impl ResolvedSettlement {
    /// Let the payer's own SOL cover native SOL payout shortfalls up to `max`.
    pub(crate) fn with_max_native_shortfall(mut self, max: Option<MaxNativeShortfall>) -> Self {
        self.max_native_shortfall = max;
        self
    }

    /// Declare `limit` compute units in place of the solver's estimate.
    pub(crate) fn with_compute_unit_limit(mut self, limit: u32) -> Self {
        self.settlement.solution.cu_estimate = Some(limit);
        self
    }

    /// Build the settlement instruction list.
    fn instructions(&self) -> Result<Vec<Instruction>, Error> {
        let payer = self.payer;
        // Prepare each order for settlement: resolve its executed amounts and
        // build its intent, sell-mint pull, and buy-mint push.
        let settlement_orders: Vec<SettlementOrder> = self
            .settlement
            .orders
            .iter()
            .map(|order| {
                let amounts = executed_amounts(order, &self.settlement.solution)?;
                let sell_program = self.token_programs.get(order.sell_token)?;
                // A native SOL buy is paid in lamports, which no token program
                // moves.
                let checked_push = !order.buys_native_sol()
                    && transfer_checked(self.token_programs.get(order.buy_token)?);
                SettlementOrder::new(order, &payer, amounts, sell_program, checked_push)
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
                        use_transfer_checked: data.checked_pull,
                    },
                    FinalizedIntent {
                        intent: &data.intent,
                        amount: data.buy_amount,
                        use_transfer_checked: data.checked_push,
                    },
                )
            })
            .unzip();
        let funding = native_payout_funding(&payer, &settlement_orders, self.max_native_shortfall)?;

        // Start populating the instruction list.
        let mut instructions = Vec::new();

        // Without an estimate the runtime's default limit applies.
        if let Some(cu_limit) = self.settlement.solution.cu_estimate {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(cu_limit));
        }
        // Insert a `CreateBuffers` instruction per token program with missing
        // buffer accounts: one instruction creates buffers under one program.
        for program in TokenProgram::ALL {
            let mut mints = Vec::new();
            for mint in &self.missing_buffers {
                if self.token_programs.get(*mint)? == program {
                    mints.push(*mint);
                }
            }
            if !mints.is_empty() {
                instructions.push(
                    CreateBuffers {
                        program_id: self.settlement.program_id,
                        payer,
                        token_program: program,
                        mints: &mints,
                    }
                    .into(),
                );
            }
        }
        // A token transfer into an uninitialized account reverts, so every ATA
        // the settlement transfers into must exist before `BeginSettle` runs.
        for ata in &self.missing_atas {
            instructions.push(create_associated_token_account_idempotent(
                &payer,
                &ata.owner,
                &ata.mint,
                self.token_programs.get(ata.mint)?,
            ));
        }

        // BeginSettle and FinalizeSettle reference each other by index, so
        // compute their positions before pushing them.
        let begin_ix_index =
            u16::try_from(instructions.len()).map_err(|_| Error::InstructionIndexOverflow)?;
        let finalize_ix_index = u16::try_from(
            instructions.len() + 1 + self.settlement.solution.interactions.len() + funding.len(),
        )
        .map_err(|_| Error::InstructionIndexOverflow)?;

        instructions.push(
            BeginSettle {
                program_id: self.settlement.program_id,
                solver: payer,
                finalize_ix_index,
                auction_id: self.settlement.auction_id.get(),
                // `None` enables both token programs, so one settlement can
                // move mints of both.
                only_token_program: None,
                orders: &initialized_intents,
                // The mint rule refuses hook programs, so no transfer needs
                // extra accounts.
                extra_transfer_accounts: &[],
            }
            .into(),
        );
        instructions.extend(self.settlement.solution.interactions.iter().cloned());
        instructions.extend(funding);
        instructions.push(
            FinalizeSettle {
                program_id: self.settlement.program_id,
                begin_ix_index,
                only_token_program: None,
                orders: &finalized_intents,
                extra_transfer_accounts: &[],
            }
            .into(),
        );

        Ok(instructions)
    }

    /// The accounts the settlement transaction locks as writable, the fee
    /// payer first: the accounts whose recent prioritization fees price it.
    pub(crate) fn writable_accounts(&self) -> Result<Vec<Pubkey>, Error> {
        let instructions = self.instructions()?;
        let written = instructions.iter().flat_map(|instruction| {
            instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_writable)
                .map(|meta| meta.pubkey)
        });
        Ok(std::iter::once(self.payer)
            .chain(written)
            .unique()
            .collect())
    }

    /// Compile the resolved settlement into the v0 message the payer signs,
    /// paying `compute_unit_price` micro-lamports per compute unit.
    fn message(&self, blockhash: Hash, compute_unit_price: u64) -> Result<MessageV0, Error> {
        let mut instructions = self.instructions()?;
        // The runtime reads compute budget instructions from anywhere in the
        // message; appending keeps the settlement's instruction indices.
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
            compute_unit_price,
        ));
        Ok(MessageV0::try_compile(
            &self.payer,
            &instructions,
            &self.lookup_tables,
            blockhash,
        )?)
    }

    /// Encode the resolved settlement as a v0 transaction signed by the payer,
    /// paying `compute_unit_price` micro-lamports per compute unit.
    pub async fn encode(
        self,
        signer: &Signer,
        blockhash: Hash,
        compute_unit_price: u64,
    ) -> Result<VersionedTransaction, Error> {
        let message = self.message(blockhash, compute_unit_price)?;
        Ok(signer.sign(message).await?)
    }

    /// The transaction [`encode`](Self::encode) signs, unsigned, for a
    /// simulation that skips signature checks. Signatures, the blockhash and
    /// the compute unit price are fixed-size, so it has the signed
    /// transaction's wire size.
    pub fn unsigned(
        &self,
        blockhash: Hash,
        compute_unit_price: u64,
    ) -> Result<VersionedTransaction, Error> {
        let message = self.message(blockhash, compute_unit_price)?;
        let signatures = vec![Signature::default(); message.header.num_required_signatures.into()];
        Ok(VersionedTransaction {
            signatures,
            message: VersionedMessage::V0(message),
        })
    }
}

/// `owner`'s associated token account for `mint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Ata {
    owner: Pubkey,
    mint: Pubkey,
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

    /// Returns a Solver's associated token account for the given mint under
    /// the mint's token program.
    fn new_ata(mint: Pubkey, owner: Pubkey, program: TokenProgram) -> Self {
        let address = associated_token_address(&owner, &mint, program);
        Self { mint, address }
    }
}

/// The orders paying out to a token account. A native SOL buy pays out to a
/// wallet, which the payout creates when it is missing.
fn token_buys(orders: &[Order]) -> impl Iterator<Item = &Order> {
    orders.iter().filter(|order| !order.buys_native_sol())
}

/// The setup accounts the settlement must create before `BeginSettle`, each
/// list sorted and deduplicated: the mints whose buffer PDA is missing on
/// chain, and the missing ATAs, both the payer's ATAs and the orders' buy
/// ATAs. `token_program` resolves the program a buy ATA derives under.
fn accounts_to_create(
    orders: &[Order],
    buffers: &[SetupAccount],
    payer_atas: &[SetupAccount],
    payer: Pubkey,
    snapshot: &AccountsSnapshot,
    token_programs: &TokenPrograms,
) -> Result<(Vec<Pubkey>, Vec<Ata>), ResolveError> {
    let mut missing_buffers = missing_setup_accounts(buffers, snapshot)?;
    missing_buffers.sort_unstable();
    missing_buffers.dedup();

    let mut missing_payer_atas = missing_setup_accounts(payer_atas, snapshot)?;
    // Settlements buying native SOL close the payer's wSOL ATA, so one in
    // flight can close it after the snapshot. Create it regardless.
    if payer_atas
        .iter()
        .any(|account| account.mint == native_mint::ID)
    {
        missing_payer_atas.push(native_mint::ID);
    }
    let missing_payer_atas = missing_payer_atas
        .into_iter()
        .map(|mint| Ata { owner: payer, mint });

    // Checked against the chain again rather than taken from the solve-time
    // resolution: an account closed since would revert the payout.
    let mut missing_atas: Vec<Ata> = missing_payer_atas.collect();
    for order in token_buys(orders) {
        if snapshot.token_account_needs_creation(order.buy_token_account)
            && order.buy_token_account_is_ata(token_programs.get(order.buy_token)?)
        {
            missing_atas.push(Ata {
                owner: order.owner,
                mint: order.buy_token,
            });
        }
    }
    missing_atas.sort_unstable();
    missing_atas.dedup();

    // The one place the payer's rent buys someone else an account, which its
    // owner can close and reclaim the rent lamports right after the fill.
    for ata in missing_atas.iter().filter(|ata| ata.owner != payer) {
        tracing::info!(
            %payer,
            owner = %ata.owner,
            mint = %ata.mint,
            "creating a user's buy token account"
        );
    }

    Ok((missing_buffers, missing_atas))
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
    /// A setup account exists on chain in a state the settlement can neither
    /// use nor create over.
    #[error("setup account {account} for mint {mint} has unexpected owner {owner}")]
    UnexpectedSetupAccount {
        account: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
    },
    /// A mint the settlement moves is missing on chain or is not a mint of
    /// either token program.
    #[error("invalid mint {mint}: {reason}")]
    InvalidMint {
        mint: Pubkey,
        reason: InvalidMintReason,
    },
    #[error(transparent)]
    UnresolvedMint(#[from] UnresolvedMint),
}

impl TryFrom<&Order> for OrderIntent {
    type Error = Error;

    /// The program refuses an intent selling or buying zero, so such an order
    /// has no intent.
    fn try_from(order: &Order) -> Result<Self, Error> {
        let amount = |amount| NonZeroU64::new(amount).ok_or(Error::ZeroAmount(order.uid));
        Ok(OrderIntent {
            owner: order.owner,
            sell: TokenAsset {
                mint: order.sell_token,
                token_account: order.sell_token_account,
            },
            buy: Asset::decode(order.buy_token, order.buy_token_account),
            sell_amount: amount(order.sell_amount)?,
            buy_amount: amount(order.buy_amount)?,
            valid_to: order.valid_to,
            // Every auction order exists as a PDA the owner created with
            // `CreateOrder`, and the program only accepts that instruction
            // for intents carrying this flag. An off-chain Ed25519 intent
            // flow would carry the flag on the wire instead.
            flags: Flags {
                created_on_chain: true,
                kind: match order.side {
                    Side::Sell => OrderKind::Sell,
                    Side::Buy => OrderKind::Buy,
                },
                partially_fillable: order.partially_fillable,
            },
            app_data: order.app_data,
        })
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
        let intent_uid = OrderIntent::try_from(order)?.uid();
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
        let available = order.available();
        let (filled, target) = match order.side {
            Side::Sell => (amounts.sell, available.sell),
            Side::Buy => (amounts.buy, available.buy),
        };

        // A non-partially-fillable order must be filled exactly.
        if !order.partially_fillable && filled != target {
            return Err(Error::NotExactlyFilled(order.uid));
        }

        // `executed` and the balance can lag a settlement that just landed,
        // so the program's cumulative check and the pull stay the authority;
        // this one only saves a doomed transaction's fees.
        if filled > target {
            return Err(Error::Overfill(order.uid));
        }

        // The executed price must not be worse than the order's limit price.
        if !respects_limit(order, amounts.sell, amounts.buy) {
            return Err(Error::LimitPriceViolated(order.uid));
        }
    }
    Ok(())
}

/// Whether the executed legs respect the order's signed limit price: the
/// executed price must not be worse than the limit. The same
/// cross-multiplication as the program's `validate_limit_price`, so nothing the
/// chain would accept is rejected here, and there is no division to round.
pub(crate) fn respects_limit(order: &Order, executed_sell: u64, executed_buy: u64) -> bool {
    u128::from(executed_buy) * u128::from(order.sell_amount)
        >= u128::from(executed_sell) * u128::from(order.buy_amount)
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
    /// Whether the sell tokens are pulled with `TransferChecked`.
    checked_pull: bool,
    /// Whether the buy tokens are pushed with `TransferChecked`.
    checked_push: bool,
}

impl SettlementOrder {
    /// Build a settlement order from a domain order: its intent, its sell-mint
    /// pull into the payer's sell ATA, and its buy-mint push, made with
    /// `TransferChecked` when `checked_push` is set.
    ///
    /// The swap output lands in the buy-mint buffer PDA, or the payer's wSOL
    /// ATA for a native SOL buy (see `infra/solver/dto/auction.rs`), so the
    /// sell tokens are pulled into the payer's sell ATA, derived under the sell
    /// mint's token program, rather than a buffer.
    fn new(
        order: &Order,
        payer: &Pubkey,
        amounts: ExecutedAmounts,
        sell_program: TokenProgram,
        checked_push: bool,
    ) -> Result<Self, Error> {
        Ok(Self {
            intent: order.try_into()?,
            pulls: vec![Pull {
                destination: associated_token_address(payer, &order.sell_token, sell_program),
                amount: amounts.sell,
            }],
            buy_amount: amounts.buy,
            checked_pull: transfer_checked(sell_program),
            checked_push,
        })
    }
}

/// The largest share of the native SOL payouts, in basis points below 100%,
/// that the payer's own SOL covers when the swap output falls short of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "u16")]
pub struct MaxNativeShortfall(u16);

#[derive(Debug, thiserror::Error)]
#[error("max native shortfall must be below {} bps, got {0}", BPS_DENOMINATOR)]
pub struct ShortfallOutOfRange(u16);

impl TryFrom<u16> for MaxNativeShortfall {
    type Error = ShortfallOutOfRange;

    fn try_from(bps: u16) -> Result<Self, Self::Error> {
        (bps < BPS_DENOMINATOR)
            .then_some(Self(bps))
            .ok_or(ShortfallOutOfRange(bps))
    }
}

impl MaxNativeShortfall {
    /// The least swap output that still funds `payouts`. Rounds up, so the
    /// payer never covers more than the capped share.
    fn required_output(self, payouts: u64) -> u64 {
        let covered = u128::from(payouts) * u128::from(self.0) / u128::from(BPS_DENOMINATOR);
        payouts - u64::try_from(covered).expect("a share below 100% of a u64 fits in u64")
    }
}

/// Whether the settlement moves a mint of `program` with `TransferChecked`.
/// Every Token-2022 mint takes it, so no extension (transfer fee, transfer
/// hook, pausable) has to be parsed to know which ones refuse a plain
/// `Transfer`. It costs the mint account and about 1300 CU per transfer.
fn transfer_checked(program: TokenProgram) -> bool {
    program == TokenProgram::Token2022
}

/// The instructions that fund the state PDA's native SOL payouts: check that
/// the payer's wSOL ATA holds the payouts less the `max_shortfall` share, close
/// it to unwrap the swap output, then transfer exactly the payouts to the
/// state PDA. The payer's own SOL covers a gap within `max_shortfall`, and the
/// check reverts the settlement on a larger one. Only pushes move lamports out
/// of the state PDA, so any excess would stay there. Empty without a native
/// SOL buy.
fn native_payout_funding(
    payer: &Pubkey,
    orders: &[SettlementOrder],
    max_shortfall: Option<MaxNativeShortfall>,
) -> Result<Vec<Instruction>, Error> {
    let mut payouts = orders
        .iter()
        .filter(|order| matches!(order.intent.buy, Asset::Native(_)))
        .map(|order| order.buy_amount)
        .peekable();
    if payouts.peek().is_none() {
        return Ok(Vec::new());
    }
    let total = payouts
        .try_fold(0, u64::checked_add)
        .ok_or(Error::ExecutedAmountOverflow)?;
    let required = max_shortfall.map_or(total, |max| max.required_output(total));
    let wsol_ata = associated_token_address(payer, &native_mint::ID, TokenProgram::SplToken);
    Ok(vec![
        require_token_balance(&wsol_ata, payer, required),
        close_token_account(&wsol_ata, payer, payer),
        transfer(payer, &STATE_PDA, total),
    ])
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
    /// The order was filled for more than its remaining amount.
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
    /// The order sells or buys zero, which no intent encodes.
    #[error("order {0} sells or buys zero")]
    ZeroAmount(OrderUid),
    /// The transaction failed to compile.
    #[error("failed to compile transaction: {0}")]
    Compile(#[from] solana_sdk::message::CompileError),
    /// The transaction failed to sign.
    #[error("failed to sign transaction: {0}")]
    Sign(#[from] cow_solana_signer::Error),
    /// The instruction index does not fit in `u16`.
    #[error("instruction index does not fit in u16")]
    InstructionIndexOverflow,
    #[error(transparent)]
    UnresolvedMint(#[from] UnresolvedMint),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::Trade,
        cow_settlement_interface::{
            data::intent::ENCODED_NATIVE_SOL_TRANSFER,
            instruction::{
                InstructionInputParsing,
                settle::{BeginSettleInput, FinalizeSettleInput, MINT_PLACEHOLDER},
            },
        },
        cow_solana_rpc::{MocksMap, RpcRequest, SolanaRPC},
        serde_json::Value,
        solana_sdk::signer::keypair::Keypair,
        solana_testlib::{
            account_json,
            mint_account_json,
            multiple_accounts_json,
            token_2022_mint,
            token_account_json,
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
            executed: 0,
            sell_balance: None,
        };
        customize(&mut order);
        let uid = OrderIntent::try_from(&order).unwrap().uid();
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
            solver_fee: 0,
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

    /// Every mint of the settlement under the SPL Token program.
    fn spl_token_programs(settlement: &Settlement) -> TokenPrograms {
        TokenPrograms(
            settlement
                .mints()
                .into_iter()
                .map(|mint| (mint, TokenProgram::SplToken))
                .collect(),
        )
    }

    /// Answers the settlement's two `getMultipleAccounts` lookups in order:
    /// `mints` for the mint accounts, then `accounts` for the lookup tables,
    /// buffer PDAs, payer ATAs and the token buys' buy token accounts, each in
    /// the settlement's request order.
    fn blockchain(
        mints: impl IntoIterator<Item = Value>,
        accounts: impl IntoIterator<Item = Value>,
    ) -> Solana {
        let mocks = MocksMap::from_iter([
            (
                RpcRequest::GetMultipleAccounts,
                multiple_accounts_json(mints),
            ),
            (
                RpcRequest::GetMultipleAccounts,
                multiple_accounts_json(accounts),
            ),
        ]);
        Solana::new(
            SolanaRPC::new_mock_with_mocks_map(mocks.clone()),
            SolanaRPC::new_mock_with_mocks_map(mocks),
            pubkey(0xaa),
        )
    }

    fn resolve_for_test(settlement: Settlement, payer: Pubkey) -> ResolvedSettlement {
        ResolvedSettlement {
            token_programs: spl_token_programs(&settlement),
            settlement,
            payer,
            lookup_tables: Vec::new(),
            missing_buffers: Vec::new(),
            missing_atas: Vec::new(),
            max_native_shortfall: None,
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
        let intent_uid = OrderIntent::try_from(&order).unwrap().uid();

        let err = test_settlement(&[order], &[trade(bogus_uid, 1_000, 2_000)])
            .expect_err("an order whose uid does not match its intent must be rejected");
        assert_eq!(err, Error::OrderIntentMismatch(intent_uid, bogus_uid));
    }

    #[test]
    fn zero_amount_orders_are_rejected() {
        let mut order = test_order(&pubkey(0xaa));
        order.sell_amount = 0;
        let err = test_settlement(&[order.clone()], &[trade(order.uid, 1_000, 2_000)])
            .expect_err("an order selling zero has no intent");
        assert_eq!(err, Error::ZeroAmount(order.uid));
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

        let instructions = resolve_for_test(settlement, payer).instructions().unwrap();
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.orders.iter().count(), 1);
    }

    /// The unsigned transaction has the signed one's wire size.
    #[tokio::test]
    async fn unsigned_has_the_signed_wire_size() {
        let program_id = pubkey(0xaa);
        let signer = Signer::Keypair(Keypair::new());
        let order = test_order(&program_id);
        let uid = order.uid;
        let settlement = test_settlement(&[order], &[trade(uid, 1_000, 2_000)]).unwrap();
        let resolved = resolve_for_test(settlement, signer.pubkey());

        let unsigned = resolved.unsigned(Hash::default(), 0).unwrap();
        let signed = resolved
            .encode(&signer, Hash::new_unique(), 1_500)
            .await
            .unwrap();
        assert_eq!(
            bincode::serialized_size(&unsigned).unwrap(),
            bincode::serialized_size(&signed).unwrap()
        );
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

        let instructions = resolve_for_test(settlement, payer).instructions().unwrap();
        // [SetComputeUnitLimit, BeginSettle, FinalizeSettle].
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        let destination = begin_input.orders.iter().next().unwrap().destinations[0];
        assert_eq!(
            destination,
            associated_token_address(&payer, &sell_token, TokenProgram::SplToken),
        );
    }

    /// Pin: a fill that respects the user's limit still validates after the
    /// solver fee shrinks the delivered buy amount. The solve-time filter
    /// in `compute_solutions` is the real guard; this settle-time check
    /// only re-validates against state drift.
    #[test]
    fn fee_adjusted_fill_that_still_respects_limit_passes_validation() {
        let program_id = pubkey(0xaa);
        // Sell 1000, user accepts as little as 1000 buy (limit 1:1). The route
        // delivers 2000 buy and the solver fee shrinks it to 1900, still above
        // the 1000 limit.
        let order = test_order_with(&program_id, |order| order.buy_amount = 1_000);
        test_settlement(
            std::slice::from_ref(&order),
            &[trade(order.uid, 1_000, 1_900)],
        )
        .expect("a fee-adjusted fill above the limit must validate");
    }

    /// Pin: a route that only fills at the user's limit is pushed below it once
    /// the solver fee is applied, and the fee-adjusted payout undercuts the
    /// signed limit. The solve-time filter drops such solutions first; this
    /// settle-time check re-validates against state drift before paying for a
    /// reverting tx.
    #[test]
    fn fee_adjusted_fill_below_limit_is_rejected() {
        let program_id = pubkey(0xaa);
        // Sell 1000, user demands at least 1900 buy. The route delivers 1900,
        // which the solver fee shrinks to 1805 — below the signed limit.
        let order = test_order_with(&program_id, |order| order.buy_amount = 1_900);
        let err = test_settlement(
            std::slice::from_ref(&order),
            &[trade(order.uid, 1_000, 1_805)],
        )
        .expect_err("a fee-adjusted fill below the limit must be rejected");
        assert_eq!(err, Error::LimitPriceViolated(order.uid));
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

        resolve_for_test(settlement, payer).instructions().unwrap();
    }

    /// The cap is what prior settlements left open, not the signed amount:
    /// 400 of 1000 sold leaves 600, so filling 600 passes and 601 is
    /// rejected.
    #[test]
    fn caps_a_partially_filled_order_at_its_remaining_amount() {
        let program_id = pubkey(0xaa);
        let order = test_order_with(&program_id, |order| {
            order.partially_fillable = true;
            order.executed = 400;
        });
        test_settlement(slice::from_ref(&order), &[trade(order.uid, 600, 1_200)])
            .expect("filling the remainder must pass");
        let err = test_settlement(slice::from_ref(&order), &[trade(order.uid, 601, 1_202)])
            .expect_err("a fill over the remainder must be rejected");
        assert_eq!(err, Error::Overfill(order.uid));
    }

    /// A buy order caps on its buy leg: 400 of 2000 bought leaves 1600, and
    /// the 1000 sell limit scales to 800. Buying 1600 passes and 1601 is
    /// rejected even though the sell leg stays within its limit.
    #[test]
    fn caps_a_partially_filled_buy_order_at_its_remaining_amount() {
        let program_id = pubkey(0xaa);
        let order = test_order_with(&program_id, |order| {
            order.side = Side::Buy;
            order.partially_fillable = true;
            order.executed = 400;
        });
        test_settlement(slice::from_ref(&order), &[trade(order.uid, 800, 1_600)])
            .expect("filling the remainder must pass");
        let err = test_settlement(slice::from_ref(&order), &[trade(order.uid, 800, 1_601)])
            .expect_err("a fill over the remainder must be rejected");
        assert_eq!(err, Error::Overfill(order.uid));
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

    /// A buy order whose executed price is worse than its limit is rejected.
    #[test]
    fn rejects_a_buy_order_that_violates_its_limit_price() {
        let program_id = pubkey(0xaa);
        // sell_amount: 1_000, buy_amount: 2_000. Executed: 600 sold / 1_000
        // bought. 1_000 * 1_000 < 600 * 2_000, so the limit price is
        // violated.
        let order = test_order_with(&program_id, |order| order.side = Side::Buy);
        assert!(!respects_limit(&order, 600, 1_000));
    }

    /// The comparison is inclusive: a fill at exactly the limit price passes
    /// on both sides.
    #[test]
    fn a_fill_at_exactly_the_limit_price_passes() {
        let program_id = pubkey(0xaa);
        let sell_order = test_order(&program_id);
        assert!(respects_limit(&sell_order, 1_000, 2_000));

        let buy_order = test_order_with(&program_id, |order| order.side = Side::Buy);
        assert!(respects_limit(&buy_order, 500, 1_000));
    }

    /// A zero limit leg: a sell order demanding nothing accepts any fill, a
    /// buy order offering nothing accepts only a free fill.
    #[test]
    fn a_zero_limit_leg_is_handled() {
        // No intent encodes a zero leg, so zero it after the uid derivation.
        let program_id = pubkey(0xaa);
        let mut sell_order = test_order(&program_id);
        sell_order.buy_amount = 0;
        assert!(respects_limit(&sell_order, 1_000, 0));

        let mut buy_order = test_order_with(&program_id, |order| order.side = Side::Buy);
        buy_order.sell_amount = 0;
        assert!(respects_limit(&buy_order, 0, 1_000));
        assert!(!respects_limit(&buy_order, 1, 1_000));
    }

    /// The comparison is exact for non-integer limit prices: one unit below
    /// the fair price fails, one unit at it passes.
    #[test]
    fn non_integer_limit_prices_have_an_exact_boundary() {
        let program_id = pubkey(0xaa);
        // Sell 3 for at least 2. Selling 1_000 must yield at least
        // ceil(2000/3) = 667.
        let sell_order = test_order_with(&program_id, |order| {
            order.sell_amount = 3;
            order.buy_amount = 2;
        });
        assert!(!respects_limit(&sell_order, 1_000, 666));
        assert!(respects_limit(&sell_order, 1_000, 667));

        // Buy 3 paying at most 2. Buying 1_000 may cost at most
        // floor(2000/3) = 666.
        let buy_order = test_order_with(&program_id, |order| {
            order.side = Side::Buy;
            order.sell_amount = 2;
            order.buy_amount = 3;
        });
        assert!(respects_limit(&buy_order, 666, 1_000));
        assert!(!respects_limit(&buy_order, 667, 1_000));
    }

    /// `u64::MAX` legs stay within `u128` and do not overflow.
    #[test]
    fn max_amounts_do_not_overflow() {
        let program_id = pubkey(0xaa);
        let order = test_order_with(&program_id, |order| {
            order.sell_amount = u64::MAX;
            order.buy_amount = u64::MAX;
        });
        assert!(respects_limit(&order, u64::MAX, u64::MAX));
        assert!(!respects_limit(&order, u64::MAX, u64::MAX - 1));
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

        let instructions = resolve_for_test(settlement, payer).instructions().unwrap();
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

        let instructions = resolve_for_test(settlement, payer).instructions().unwrap();
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
        let missing_atas = vec![Ata {
            owner: payer,
            mint: order.sell_token,
        }];
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order],
            solution(trades),
        )
        .unwrap();
        let resolved = ResolvedSettlement {
            token_programs: spl_token_programs(&settlement),
            settlement,
            payer,
            lookup_tables: Vec::new(),
            missing_buffers,
            missing_atas,
            max_native_shortfall: None,
        };

        let instructions = resolved.instructions().unwrap();

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

    /// Both mints are SPL Token mints. The account lookup answers in order:
    /// buffer PDA, payer sell ATA and the order's buy account, all absent. The
    /// buy account is the owner's associated token account, so it joins the
    /// accounts to create.
    #[tokio::test]
    async fn resolves_a_missing_buy_ata_as_an_account_to_create() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order_with(&program_id, |order| {
            order.buy_token_account =
                associated_token_address(&order.owner, &order.buy_token, TokenProgram::SplToken);
        });
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();

        let resolved = settlement
            .resolve_accounts(
                &blockchain(
                    [mint_account_json(), mint_account_json()],
                    [Value::Null, Value::Null, Value::Null],
                ),
                payer,
            )
            .await
            .unwrap();

        assert_eq!(resolved.missing_buffers, vec![order.buy_token]);
        let mut expected = vec![
            Ata {
                owner: payer,
                mint: order.sell_token,
            },
            Ata {
                owner: order.owner,
                mint: order.buy_token,
            },
        ];
        expected.sort_unstable();
        assert_eq!(resolved.missing_atas, expected);
    }

    /// The sell mint and wSOL are SPL Token mints. The account lookup answers
    /// in order: the payer's sell ATA and wSOL ATA, both absent. A native SOL
    /// buy has no buffer and no buy account to look up or create; the payer's
    /// wSOL ATA is created for the swap output.
    #[tokio::test]
    async fn a_native_sol_buy_adds_the_payers_wsol_ata_and_no_buy_ata() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = native_sol_order(&program_id, pubkey(0x68), pubkey(0x45));
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();

        let resolved = settlement
            .resolve_accounts(
                &blockchain(
                    [mint_account_json(), mint_account_json()],
                    [Value::Null, Value::Null],
                ),
                payer,
            )
            .await
            .unwrap();

        assert!(resolved.missing_buffers.is_empty());
        let mut expected = vec![
            Ata {
                owner: payer,
                mint: order.sell_token,
            },
            Ata {
                owner: payer,
                mint: native_mint::ID,
            },
        ];
        expected.sort_unstable();
        assert_eq!(resolved.missing_atas, expected);
    }

    #[test]
    fn creates_the_orders_buy_ata_for_its_owner() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();
        let resolved = ResolvedSettlement {
            token_programs: spl_token_programs(&settlement),
            settlement,
            payer,
            lookup_tables: Vec::new(),
            missing_buffers: Vec::new(),
            missing_atas: vec![Ata {
                owner: order.owner,
                mint: order.buy_token,
            }],
            max_native_shortfall: None,
        };

        let instructions = resolved.instructions().unwrap();

        // [SetComputeUnitLimit, CreateAtaIdempotent, BeginSettle,
        // FinalizeSettle].
        assert_eq!(instructions.len(), 4);
        let create = &instructions[1];
        assert_eq!(
            create.program_id,
            spl_associated_token_account_interface::program::ID
        );
        let accounts: Vec<Pubkey> = create.accounts.iter().map(|m| m.pubkey).collect();
        assert_eq!(accounts[0], payer);
        assert_eq!(
            accounts[1],
            associated_token_address(&order.owner, &order.buy_token, TokenProgram::SplToken)
        );
        assert_eq!(accounts[2], order.owner);
        assert_eq!(accounts[3], order.buy_token);
    }

    /// The fee payer leads, every account an instruction writes follows once,
    /// and read-only accounts such as program ids stay out.
    #[test]
    fn writable_accounts_are_the_payer_and_the_written_accounts() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();

        let writable = resolve_for_test(settlement, payer)
            .writable_accounts()
            .unwrap();

        assert_eq!(writable[0], payer);
        assert_eq!(writable.iter().unique().count(), writable.len());
        assert!(writable.contains(&order.sell_token_account));
        assert!(!writable.contains(&program_id));
        assert!(!writable.contains(&solana_compute_budget_interface::ID));
    }

    /// `encode` appends `SetComputeUnitPrice` after the settlement's
    /// instructions.
    #[tokio::test]
    async fn encode_appends_the_compute_unit_price() {
        let program_id = pubkey(0xaa);
        let signer = Signer::Keypair(Keypair::new());
        let order = test_order(&program_id);
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();
        let resolved = resolve_for_test(settlement, signer.pubkey());
        let settlement_instructions = resolved.instructions().unwrap().len();

        let transaction = resolved
            .encode(&signer, Hash::default(), 1_500)
            .await
            .unwrap();

        let message = &transaction.message;
        assert_eq!(message.instructions().len(), settlement_instructions + 1);
        let last = message.instructions().last().unwrap();
        let price = ComputeBudgetInstruction::set_compute_unit_price(1_500);
        assert_eq!(
            message.static_account_keys()[usize::from(last.program_id_index)],
            price.program_id
        );
        assert_eq!(last.data, price.data);
    }

    /// A sell order buying native SOL into `wallet`, with its own sell token so
    /// every such order gets its own uid.
    fn native_sol_order(program_id: &Pubkey, wallet: Pubkey, sell_token: Pubkey) -> Order {
        test_order_with(program_id, |order| {
            order.buy_token = ENCODED_NATIVE_SOL_TRANSFER;
            order.buy_token_account = wallet;
            order.sell_token = sell_token;
        })
    }

    /// A native SOL buy has no buffer PDA. Its swap output lands in the payer's
    /// wSOL ATA, so that ATA joins the payer's sell ATAs.
    #[test]
    fn a_native_sol_buy_needs_the_payers_wsol_ata_instead_of_a_buffer() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let token_buy = test_order(&program_id);
        let native_buy = native_sol_order(&program_id, pubkey(0x68), pubkey(0x45));
        let settlement = test_settlement(
            &[token_buy.clone(), native_buy.clone()],
            &[
                trade(token_buy.uid, 1_000, 2_000),
                trade(native_buy.uid, 1_000, 2_000),
            ],
        )
        .unwrap();

        let (buffers, payer_atas) = settlement
            .setup_accounts(payer, &spl_token_programs(&settlement))
            .unwrap();
        let buffer_mints: Vec<Pubkey> = buffers.iter().map(|account| account.mint).collect();
        assert_eq!(buffer_mints, vec![token_buy.buy_token]);
        let mut ata_mints: Vec<Pubkey> = payer_atas.iter().map(|account| account.mint).collect();
        ata_mints.sort_unstable();
        let mut expected = vec![token_buy.sell_token, native_buy.sell_token, native_mint::ID];
        expected.sort_unstable();
        assert_eq!(ata_mints, expected);
    }

    /// Between the interactions and `FinalizeSettle`, the payer checks that its
    /// wSOL ATA holds the native SOL payouts, unwraps it and moves exactly the
    /// payouts into the state PDA, which `FinalizeSettle` pays those orders
    /// from. Token payouts are not part of the transfer.
    #[test]
    fn native_sol_payouts_are_funded_from_the_unwrapped_wsol() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let (wallet_a, wallet_b) = (pubkey(0x68), pubkey(0x69));
        let token_buy = test_order(&program_id);
        let native_a = native_sol_order(&program_id, wallet_a, pubkey(0x45));
        let native_b = test_order_with(&program_id, |order| {
            order.buy_token = ENCODED_NATIVE_SOL_TRANSFER;
            order.buy_token_account = wallet_b;
            order.sell_token = pubkey(0x46);
            order.sell_amount = 500;
            order.buy_amount = 1_000;
        });
        let settlement = test_settlement(
            &[token_buy.clone(), native_a.clone(), native_b.clone()],
            &[
                trade(token_buy.uid, 1_000, 2_000),
                trade(native_a.uid, 1_000, 2_000),
                trade(native_b.uid, 500, 1_000),
            ],
        )
        .unwrap();

        let instructions = resolve_for_test(settlement, payer).instructions().unwrap();
        // [SetComputeUnitLimit, BeginSettle, Transfer (self), CloseAccount,
        // Transfer, FinalizeSettle].
        assert_eq!(instructions.len(), 6);
        let state_pda = STATE_PDA;
        let wsol_ata = associated_token_address(&payer, &native_mint::ID, TokenProgram::SplToken);
        assert_eq!(
            instructions[2],
            spl_token_interface::instruction::transfer(
                &spl_token_interface::ID,
                &wsol_ata,
                &wsol_ata,
                &payer,
                &[],
                3_000,
            )
            .unwrap()
        );
        assert_eq!(
            instructions[3],
            close_token_account(&wsol_ata, &payer, &payer)
        );
        assert_eq!(instructions[4], transfer(&payer, &state_pda, 3_000));

        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(begin_input.finalize_ix_index, 5);

        let finalize = &instructions[5];
        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        assert_eq!(finalize_input.begin_ix_index, 1);
        let mut native_pushes: Vec<(Pubkey, u64)> = finalize_input
            .pushes
            .iter()
            .filter(|push| *push.source_buffer == state_pda)
            .map(|push| (*push.destination, push.amount))
            .collect();
        native_pushes.sort_unstable();
        assert_eq!(native_pushes, vec![(wallet_a, 2_000), (wallet_b, 1_000)]);
    }

    /// With a shortfall cap, the wSOL check asks for the payouts less the
    /// capped share while the transfer still moves the full payouts, so the
    /// payer's own SOL covers the gap.
    #[test]
    fn a_capped_native_sol_shortfall_is_covered_by_the_payer() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = native_sol_order(&program_id, pubkey(0x68), pubkey(0x45));
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();

        let instructions = resolve_for_test(settlement, payer)
            .with_max_native_shortfall(Some(MaxNativeShortfall::try_from(40).unwrap()))
            .instructions()
            .unwrap();
        // [SetComputeUnitLimit, BeginSettle, Transfer (self), CloseAccount,
        // Transfer, FinalizeSettle]. 40 bps of 2_000 is 8.
        let wsol_ata = associated_token_address(&payer, &native_mint::ID, TokenProgram::SplToken);
        assert_eq!(
            instructions[2],
            spl_token_interface::instruction::transfer(
                &spl_token_interface::ID,
                &wsol_ata,
                &wsol_ata,
                &payer,
                &[],
                1_992,
            )
            .unwrap()
        );
        assert_eq!(instructions[4], transfer(&payer, &STATE_PDA, 2_000));
    }

    /// The payer covers at most the capped share of the payouts, and the
    /// largest whole amount within it.
    #[test]
    fn required_output_is_the_exact_boundary_of_the_cap() {
        for bps in [0u16, 1, 40, 9_999] {
            let max = MaxNativeShortfall::try_from(bps).unwrap();
            for payouts in [0u64, 1, 2, 9_999, 10_000, 123_456_789, u64::MAX] {
                let required = max.required_output(payouts);
                assert!(required <= payouts, "bps={bps} payouts={payouts}");
                let covered = u128::from(payouts - required);
                let cap = u128::from(payouts) * u128::from(bps);
                assert!(covered * 10_000 <= cap, "bps={bps} payouts={payouts}");
                assert!((covered + 1) * 10_000 > cap, "bps={bps} payouts={payouts}");
            }
        }
    }

    /// The settlement reads the mint of every sell and token buy, plus wSOL
    /// for a native SOL buy, never the native SOL marker itself.
    #[test]
    fn settlement_mints_include_wsol_for_native_sol_buys() {
        let program_id = pubkey(0xaa);
        let token_buy = test_order(&program_id);
        let native_buy = native_sol_order(&program_id, pubkey(0x68), pubkey(0x45));
        let settlement = test_settlement(
            &[token_buy.clone(), native_buy.clone()],
            &[
                trade(token_buy.uid, 1_000, 2_000),
                trade(native_buy.uid, 1_000, 2_000),
            ],
        )
        .unwrap();

        assert_eq!(
            settlement.mints(),
            HashSet::from([
                token_buy.sell_token,
                token_buy.buy_token,
                native_buy.sell_token,
                native_mint::ID,
            ])
        );
    }

    /// Token-2022 mints settle beside SPL Token ones: each token program gets
    /// its own `CreateBuffers`, and the payer's sell ATAs and the pulls into
    /// them derive under the sell mint's program.
    #[test]
    fn token_2022_mints_settle_under_their_own_program() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let spl = test_order(&program_id);
        let token_2022 = test_order_with(&program_id, |order| {
            order.sell_token = pubkey(0x45);
            order.buy_token = pubkey(0x46);
            order.sell_token_account = pubkey(0x67);
            order.buy_token_account = pubkey(0x68);
        });
        let settlement = test_settlement(
            &[spl.clone(), token_2022.clone()],
            &[
                trade(spl.uid, 1_000, 2_000),
                trade(token_2022.uid, 1_000, 2_000),
            ],
        )
        .unwrap();
        let resolved = ResolvedSettlement {
            settlement,
            payer,
            lookup_tables: Vec::new(),
            missing_buffers: vec![spl.buy_token, token_2022.buy_token],
            missing_atas: vec![
                Ata {
                    owner: payer,
                    mint: spl.sell_token,
                },
                Ata {
                    owner: payer,
                    mint: token_2022.sell_token,
                },
            ],
            token_programs: TokenPrograms(HashMap::from([
                (spl.sell_token, TokenProgram::SplToken),
                (spl.buy_token, TokenProgram::SplToken),
                (token_2022.sell_token, TokenProgram::Token2022),
                (token_2022.buy_token, TokenProgram::Token2022),
            ])),
            max_native_shortfall: None,
        };

        let instructions = resolved.instructions().unwrap();

        // [SetComputeUnitLimit, CreateBuffers (SPL Token), CreateBuffers
        // (Token-2022), CreateAtaIdempotent x2, BeginSettle, FinalizeSettle].
        assert_eq!(instructions.len(), 7);
        let create_buffers = |token_program, mint| -> Instruction {
            CreateBuffers {
                program_id,
                payer,
                token_program,
                mints: &[mint],
            }
            .into()
        };
        assert_eq!(
            instructions[1],
            create_buffers(TokenProgram::SplToken, spl.buy_token)
        );
        assert_eq!(
            instructions[2],
            create_buffers(TokenProgram::Token2022, token_2022.buy_token)
        );
        assert_eq!(
            instructions[3],
            create_associated_token_account_idempotent(
                &payer,
                &payer,
                &spl.sell_token,
                TokenProgram::SplToken,
            )
        );
        assert_eq!(
            instructions[4],
            create_associated_token_account_idempotent(
                &payer,
                &payer,
                &token_2022.sell_token,
                TokenProgram::Token2022,
            )
        );

        // The pulls land in the payer's ATAs under each mint's program, and
        // only the Token-2022 order carries its sell mint for TransferChecked.
        let begin = &instructions[5];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        let mut pulls: Vec<(Pubkey, Pubkey)> = begin_input
            .orders
            .iter()
            .map(|order| (order.destinations[0], *order.sell_mint))
            .collect();
        pulls.sort_unstable();
        let mut expected = vec![
            (
                associated_token_address(&payer, &spl.sell_token, TokenProgram::SplToken),
                MINT_PLACEHOLDER,
            ),
            (
                associated_token_address(&payer, &token_2022.sell_token, TokenProgram::Token2022),
                token_2022.sell_token,
            ),
        ];
        expected.sort_unstable();
        assert_eq!(pulls, expected);

        // Likewise only the Token-2022 push carries its buy mint.
        let finalize = &instructions[6];
        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        let mut pushes: Vec<(Pubkey, Pubkey)> = finalize_input
            .pushes
            .iter()
            .map(|push| (*push.destination, *push.mint))
            .collect();
        pushes.sort_unstable();
        let mut expected = vec![
            (spl.buy_token_account, MINT_PLACEHOLDER),
            (token_2022.buy_token_account, token_2022.buy_token),
        ];
        expected.sort_unstable();
        assert_eq!(pushes, expected);
    }

    /// A Token-2022 sell paid out in native SOL: the pull carries the sell
    /// mint for `TransferChecked`, the lamport payout carries the placeholder
    /// since no token program moves it.
    #[test]
    fn native_sol_payouts_are_never_transfer_checked() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let wallet = pubkey(0x68);
        let order = native_sol_order(&program_id, wallet, pubkey(0x45));
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();
        let resolved = ResolvedSettlement {
            settlement,
            payer,
            lookup_tables: Vec::new(),
            missing_buffers: Vec::new(),
            missing_atas: Vec::new(),
            token_programs: TokenPrograms(HashMap::from([
                (order.sell_token, TokenProgram::Token2022),
                (native_mint::ID, TokenProgram::SplToken),
            ])),
            max_native_shortfall: None,
        };

        let instructions = resolved.instructions().unwrap();
        // [SetComputeUnitLimit, BeginSettle, Transfer (self), CloseAccount,
        // Transfer, FinalizeSettle].
        let begin = &instructions[1];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(
            *begin_input.orders.iter().next().unwrap().sell_mint,
            order.sell_token
        );
        let finalize = &instructions[5];
        let finalize_accounts: Vec<Pubkey> = finalize.accounts.iter().map(|m| m.pubkey).collect();
        let finalize_input =
            FinalizeSettleInput::parse(&finalize.data, &finalize_accounts).unwrap();
        let push = finalize_input.pushes.iter().next().unwrap();
        assert_eq!((*push.destination, *push.mint), (wallet, MINT_PLACEHOLDER));
    }

    /// An order trading two Token-2022 mints, paying out to the owner's
    /// Token-2022 associated token account. The chain answers both mints as
    /// Token-2022 and every setup account as absent, so the settlement creates
    /// the buffer and both ATAs under Token-2022.
    #[tokio::test]
    async fn a_token_2022_order_settles_under_token_2022() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order_with(&program_id, |order| {
            order.buy_token_account =
                associated_token_address(&order.owner, &order.buy_token, TokenProgram::Token2022);
        });
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();
        let mint = account_json(&token_2022_mint(&[], |_| ()));

        let resolved = settlement
            .resolve_accounts(
                &blockchain(
                    [mint.clone(), mint],
                    [Value::Null, Value::Null, Value::Null],
                ),
                payer,
            )
            .await
            .unwrap();
        let instructions = resolved.instructions().unwrap();

        // [SetComputeUnitLimit, CreateBuffers, CreateAtaIdempotent x2,
        // BeginSettle, FinalizeSettle].
        assert_eq!(instructions.len(), 6);
        assert_eq!(
            instructions[1],
            CreateBuffers {
                program_id,
                payer,
                token_program: TokenProgram::Token2022,
                mints: &[order.buy_token],
            }
            .into()
        );
        for (owner, mint) in [(payer, order.sell_token), (order.owner, order.buy_token)] {
            assert!(
                instructions[2..4].contains(&create_associated_token_account_idempotent(
                    &payer,
                    &owner,
                    &mint,
                    TokenProgram::Token2022
                ))
            );
        }
        let begin = &instructions[4];
        let begin_accounts: Vec<Pubkey> = begin.accounts.iter().map(|m| m.pubkey).collect();
        let begin_input = BeginSettleInput::parse(&begin.data, &begin_accounts).unwrap();
        assert_eq!(
            begin_input.orders.iter().next().unwrap().destinations[0],
            associated_token_address(&payer, &order.sell_token, TokenProgram::Token2022)
        );
    }

    /// The mint lookup answers absent for both mints, then a token account at
    /// both mints' addresses. Each fails the resolution with the matching
    /// reason.
    #[tokio::test]
    async fn an_invalid_mint_fails_the_resolution() {
        let program_id = pubkey(0xaa);
        let payer = pubkey(0xbb);
        let order = test_order(&program_id);
        let settlement =
            test_settlement(slice::from_ref(&order), &[trade(order.uid, 1_000, 2_000)]).unwrap();
        let token_account = token_account_json(&order.sell_token, &order.owner);

        for (mints, expected) in [
            (
                [Value::Null, Value::Null],
                InvalidMintReason::AccountNotFound,
            ),
            (
                [token_account.clone(), token_account],
                InvalidMintReason::NotAMint,
            ),
        ] {
            let Err(err) = settlement
                .clone()
                .resolve_accounts(&blockchain(mints, []), payer)
                .await
            else {
                panic!("an invalid mint fails the resolution");
            };
            assert!(
                matches!(err, ResolveError::InvalidMint { mint, reason } if [order.sell_token, order.buy_token].contains(&mint) && reason == expected),
                "{err}"
            );
        }
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
