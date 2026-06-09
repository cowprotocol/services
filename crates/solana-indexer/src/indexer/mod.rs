//! Consumer components of the Solana settlement indexer.

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
