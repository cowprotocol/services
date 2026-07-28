#![expect(dead_code)]
//! Recovery-flow types: PDA snapshots and the options struct for
//! `getSignaturesForAddress` backfills.

use {crate::types::order::OrderUid, solana_sdk::pubkey::Pubkey};

/// Current on-chain snapshot of an order PDA, read by `getAccountInfo` for
/// reconciliation.
#[derive(Debug, Clone)]
pub(crate) struct PdaSnapshot {
    /// Order UID.
    pub order_uid: OrderUid,
    /// Cumulative `amount_withdrawn` for the order.
    pub amount_withdrawn: u64,
    /// Cumulative `amount_received` for the order.
    pub amount_received: u64,
    /// `true` if the order has been cancelled on-chain.
    pub cancelled: bool,
    /// Cancellation timestamp (Unix seconds), if cancelled.
    pub cancellation_timestamp: Option<i64>,
}

/// Options for the `getSignaturesForAddress` RPC used by the recovery backfill.
#[derive(Debug, Clone, Default)]
pub(crate) struct GetSignaturesOpts {
    /// Start slot (inclusive). `None` means "from the tip".
    pub from_slot: Option<u64>,
    /// End slot (inclusive). `None` means "to the tip".
    pub to_slot: Option<u64>,
    /// Cap on the number of signatures returned.
    pub limit: Option<usize>,
    /// Optional address filter (used when back-filling both programs
    /// in a single pass).
    pub address: Option<Pubkey>,
}
