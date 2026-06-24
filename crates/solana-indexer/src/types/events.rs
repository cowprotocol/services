//! Domain event taxonomy.
//!
//! The settlement program and SolFlow each have their own enum
//! (`SettlementEvent`, `SolFlowEvent`); the decoder's handoff to the
//! persistence step is the sum [`DecodedEvent`]. Per-order accounting
//! is reconstructed from [`TradeDelta`] snapshots.

use {
    crate::types::{OrderUid, Signature, Slot},
    solana_sdk::pubkey::Pubkey,
};

/// Change in a single order's `amount_withdrawn` and `amount_received`
/// between two consecutive account snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeDelta {
    /// Order UID this delta applies to.
    pub order_uid: OrderUid,
    /// Change in `amount_withdrawn` since the previous snapshot.
    pub amount_withdrawn_delta: u64,
    /// Change in `amount_received` since the previous snapshot.
    pub amount_received_delta: u64,
    /// Whether the order is fully filled after this trade.
    ///
    /// This is **not** a field emitted by the settlement program's event data;
    /// it is inferred by the decoder from the order PDA's post-trade snapshot.
    /// It is `true` when post-trade `amount_withdrawn` equals the order's full
    /// sell amount, or `amount_received` equals the full buy amount.
    pub order_fulfilled: bool,
}

/// Settlement-program events decoded from on-chain instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementEvent {
    /// A new order was created on-chain.
    OrderCreated {
        /// Order UID this order is identified by.
        order_uid: OrderUid,
        /// Owner of the order.
        owner: Pubkey,
        /// Address that created the order (relayer / solver).
        created_by: Pubkey,
    },
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
    SettlementFinalized {
        /// Auction id this settlement belongs to.
        auction_id: u64,
        /// Solver that won the auction.
        solver: Pubkey,
        /// Transaction signature.
        tx_signature: Signature,
        /// Slot the settlement was observed at.
        slot: Slot,
        /// Per-order accounting deltas.
        trades: Vec<TradeDelta>,
    },
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
        ix_index: u8,
    },
}

/// SolFlow-side events, populates the `solana.sol_flow` table.
///
/// Note: the paired `solana.orders` row for `OrderEnabled` is written by the
/// settlement-program decode path, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolFlowEvent {
    /// A new order was created on SolFlow.
    OrderCreated {
        /// Custodial PDA that holds the wSOL for this order.
        custodial_pda: Pubkey,
        /// Real owner of the order.
        real_owner: Pubkey,
        /// Order UID.
        order_uid: OrderUid,
        /// From `meta.post_token_balances` on the custodial wSOL
        /// account.
        sol_amount: u64,
    },
    /// An order was enabled (custody transferred to settlement program).
    OrderEnabled {
        /// Custodial PDA.
        custodial_pda: Pubkey,
        /// Address that enabled the order.
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
pub enum DecodedEvent {
    /// A settlement-program event.
    Settlement(SettlementEvent),
    /// A SolFlow event.
    SolFlow(SolFlowEvent),
}
