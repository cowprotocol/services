#![expect(dead_code)]
//! Domain event taxonomy.
//!
//! The settlement program and SolFlow each have their own enum
//! (`SettlementEvent`, `SolFlowEvent`); the decoder's handoff to the
//! persistence step is the sum [`DecodedEvent`]. Per-order accounting is
//! reconstructed from [`TradeDelta`] snapshots.

use {
    crate::types::{Signature, order::OrderUid, slot::Slot},
    solana_sdk::pubkey::Pubkey,
};

/// Change in a single order's `amount_withdrawn` and `amount_received` between
/// two consecutive account snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TradeDelta {
    /// Order UID this delta applies to.
    pub order_uid: OrderUid,
    /// Change in `amount_withdrawn` since the previous snapshot.
    pub amount_withdrawn_delta: u64,
    /// Change in `amount_received` since the previous snapshot.
    pub amount_received_delta: u64,
    /// Whether the order is fully filled after this trade.
    ///
    /// Not a field of the program's event data: the decoder infers it from
    /// the order PDA's post-trade snapshot.
    /// It is `true` when post-trade `amount_withdrawn` equals the order's full
    /// sell amount, or `amount_received` equals the full buy amount.
    pub order_fulfilled: bool,
}

/// Whether an order sells an exact amount or buys an exact amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrderKind {
    Sell,
    Buy,
}

/// A created order's full intent. The indexer is the only writer of the
/// `solana.orders` row for orders placed directly on chain, so the event
/// carries everything that row needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatedOrder {
    /// Transaction the order was created in, the replay key when indexing
    /// it fails partway.
    pub signature: Signature,
    /// Order UID this order is identified by.
    pub order_uid: OrderUid,
    /// Owner of the order.
    pub owner: Pubkey,
    /// Address that created the order (relayer / solver).
    pub created_by: Pubkey,
    /// Canonical order PDA address.
    pub order_pda: Pubkey,
    /// Account the sell amount is pulled from. The intent names token
    /// accounts, not mints: mints require an account lookup.
    pub sell_token_account: Pubkey,
    /// Account the buy amount is pushed to.
    pub buy_token_account: Pubkey,
    /// Amount sold, in the sell token's native units.
    pub sell_amount: u64,
    /// Amount bought, in the buy token's native units.
    pub buy_amount: u64,
    /// Expiry as unix seconds.
    pub valid_to: u32,
    /// Sell or buy order.
    pub kind: OrderKind,
    /// Whether partial fills are allowed.
    pub partially_fillable: bool,
    /// App-data hash carried by the intent.
    pub app_data: [u8; 32],
}

/// A finalized settlement: one `BeginSettle`/`FinalizeSettle` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FinalizedSettlement {
    /// Auction id this settlement belongs to.
    pub auction_id: i64,
    /// Solver that won the auction.
    pub solver: Pubkey,
    /// Transaction signature.
    pub tx_signature: Signature,
    /// Slot the settlement was observed at.
    pub slot: Slot,
    /// Top-level index of the `BeginSettle` instruction, part of the
    /// trade rows' primary key.
    pub instruction_index: u32,
    /// Per-order accounting deltas.
    pub trades: Vec<TradeDelta>,
}

/// Settlement-program events decoded from on-chain instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettlementEvent {
    /// A new order was created on-chain. Boxed: the full intent dwarfs the
    /// other variants.
    OrderCreated(Box<CreatedOrder>),
    /// An order was closed.
    OrderClosed {
        /// Order UID this order is identified by.
        order_uid: OrderUid,
    },
    /// An order was cancelled.
    OrderCancelled {
        /// Order UID this order is identified by.
        order_uid: OrderUid,
    },
    /// A settlement was finalized on-chain.
    SettlementFinalized(FinalizedSettlement),
    /// A new buffer PDA was created.
    BufferCreated {
        /// Token the buffer is denominated in.
        token: Pubkey,
    },
    /// A buffer PDA was used by a transaction.
    BufferUsed {
        /// Token the buffer is denominated in.
        token: Pubkey,
        /// Transaction signature that consumed the buffer.
        tx_signature: Signature,
    },
    /// A manager was updated (e.g. ownership rotation).
    ManagerUpdated {
        /// Previous manager.
        from: Pubkey,
        /// New manager.
        to: Pubkey,
    },
    /// A solver was added to the allow-list.
    SolverAdded {
        /// Solver that was added.
        solver: Pubkey,
    },
    /// A solver was removed from the allow-list.
    SolverRemoved {
        /// Solver that was removed.
        solver: Pubkey,
    },
    /// Generic solver interaction (instruction observed but not decoded into
    /// one of the structured events above).
    SolverInteraction {
        /// Transaction signature.
        tx_signature: Signature,
        /// Index of the instruction within the transaction.
        ix_index: u16,
    },
}

/// SolFlow-side events, populates the `solana.sol_flow` table.
///
/// Note: the paired `solana.orders` row for `OrderEnabled` is written by the
/// settlement-program decode path, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum SolFlowEvent {
    /// A new order was created on SolFlow.
    OrderCreated {
        /// Custodial PDA that holds the wSOL for this order.
        custodial_pda: Pubkey,
        /// Real owner of the order.
        real_owner: Pubkey,
        /// Order UID.
        order_uid: OrderUid,
        /// From `meta.post_token_balances` on the custodial wSOL account.
        sol_amount: u64,
    },
    /// A `SetUpOrder` instruction on the SolFlow program was observed.
    ///
    /// The SolFlow order's custodial wSOL PDA has been linked to a
    /// settlement-program order via CPI. At this point custody of the wrapped
    /// SOL has effectively been transferred to the settlement program, so the
    /// SolFlow order is now eligible to be included in auctions and solved.
    ///
    /// The `enabler` is the signer of the `SetUpOrder` instruction — an
    /// unprivileged relayer or participant that pays to set up the SolFlow
    /// order, not the SolFlow program account. The settlement program records
    /// the on-chain order's `created_by` as this enabler address.
    OrderEnabled {
        /// Custodial PDA.
        custodial_pda: Pubkey,
        /// Signer of the `SetUpOrder` instruction that enabled the order.
        enabler: Pubkey,
        /// Order UID.
        order_uid: OrderUid,
    },
    /// An order was recovered (e.g. after a stuck-state cleanup).
    OrderRecovered {
        /// Custodial PDA.
        custodial_pda: Pubkey,
        /// Slot the recovery was observed at.
        slot: Slot,
    },
}

/// Sum of the two program-side event enums for the persistence step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedEvent {
    /// A settlement-program event.
    Settlement(SettlementEvent),
    /// A SolFlow event.
    SolFlow(SolFlowEvent),
}
