//! Consumer components of the Solana settlement indexer.
//!
//! The four components and their roles:
//!
//! - [`Ingester`]: subscribes to the Yellowstone gRPC stream and drains it as
//!   fast as updates arrive, forwarding them to the decoder. It does no
//!   decoding itself, so the socket never backs up behind slow processing. It
//!   is also the single writer of the "latest chain slot" counter that the
//!   other components use to know how far the chain has advanced.
//!
//! - [`Decoder`]: receives the raw stream updates, picks out transactions
//!   belonging to the settlement and SolFlow programs, matches each transaction
//!   with its corresponding account-update snapshot, and persists the resulting
//!   typed events to the store.
//!
//! - [`PartialEventWatchdog`]: some events arrive in two halves (a transaction
//!   update and an account update) that don't always land together. The decoder
//!   parks the half it has in a map shared with the watchdog; the watchdog
//!   periodically scans that map and dead-letters any entry whose other half
//!   never showed up within the slot window, recording which half went missing.
//!
//! - [`FinalizationWorker`]: rows are first written at the `confirmed`
//!   commitment level. This worker re-checks them against the chain and
//!   promotes them to `finalized`, or marks them rolled back if the transaction
//!   disappeared. It uses a cheap batched RPC call for recent rows and falls
//!   back to one-call-per-row lookups for rows old enough that the batched
//!   method no longer reports them.

pub mod decoder;
pub mod finalization;
pub mod ingester;
pub mod watchdog;

pub use {
    decoder::Decoder,
    finalization::FinalizationWorker,
    ingester::Ingester,
    watchdog::PartialEventWatchdog,
};
