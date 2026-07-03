#![expect(dead_code)]
//! The ingester owns the yellowstone gRPC stream. It drains the socket as fast
//! as yellowstone delivers, pushes tagged updates into the channel, and updates
//! `LATEST_CHAIN_SLOT` on every slot-filter message. It performs no decoding.

//  TODO: This file only declares the component skeleton. The `run` body is
// `unimplemented!`; the actual drain and reconnect with backoff logic arrives
// in a later change.

use {
    crate::{persistence::Persistence, types::channel::StreamUpdate},
    std::sync::atomic::AtomicU64,
    tokio::sync::mpsc::Sender,
    yellowstone_grpc_client::GrpcConnector,
};

/// The sole writer is the ingester, on every slot-filter message. Anchors the
/// partial-event watchdog and the finalization worker. Cold start is zero; the
/// watchdog skips its comparison on the first tick.
///
/// This is the chain tip, not indexing progress. How far the indexer has
/// actually persisted is the watermark in `solana.indexer_state`, written by
/// the decoder, which is a separate value.
pub static LATEST_CHAIN_SLOT: AtomicU64 = AtomicU64::new(0);

/// Cap on the exponential backoff between reconnect attempts.
pub const RECONNECT_BACKOFF_CAP: std::time::Duration = std::time::Duration::from_secs(30);

/// Capacity of the channel from the ingester to the decoder.
pub const INGEST_TO_DECODER_CAPACITY: usize = 1024;

/// Ingester component.
///
/// Generic over a `GrpcConnector` implementor so the unit tests can drive it
/// with a mock.
pub(crate) struct Ingester<C: GrpcConnector> {
    /// gRPC connector implementor
    pub connector: C,

    /// Sends `StreamUpdate` to the decoder. Should be bounded to
    /// `INGEST_TO_DECODER_CAPACITY` entries.
    pub tx: Sender<StreamUpdate>,

    /// Persistence layer, used to checkpoint the slot.
    pub persistence: Persistence,
}

impl<C: GrpcConnector> Ingester<C> {
    /// Construct a new ingester. The caller owns the channel capacity decision.
    pub fn new(connector: C, tx: Sender<StreamUpdate>, persistence: Persistence) -> Self {
        Self {
            connector,
            tx,
            persistence,
        }
    }

    /// TODO: Outer loop: open the subscription, drain it, push into the
    /// channel, reconnect on failure with exponential backoff.
    pub async fn run(&mut self) {
        unimplemented!()
    }
}
