#![expect(dead_code)]
//! The decoder pulls `StreamUpdate`s from the ingester, decodes
//! settlement-program and SolFlow transactions, joins account-update snapshots,
//! and persists typed events.

// TODO: This file only declares the component skeleton. The `run` body is
// `unimplemented!`; the dispatch logic and persist path arrive in a later
// change.

use {
    crate::{
        persistence::Persistence,
        types::{
            channel::{PartialEvent, PartialEventKey, StreamUpdate},
            errors::PersistenceError,
        },
    },
    dashmap::DashMap,
    solana_sdk::pubkey::Pubkey,
    std::sync::Arc,
    tokio::sync::mpsc::Receiver,
};

/// Decoder component.
///
/// The watchdog holds a clone of the same `partials` map, so the two operate on
/// the same concurrent map without any message passing between them.
pub(crate) struct Decoder {
    /// Persistence layer.
    pub persistence: Persistence,

    /// Incoming `StreamUpdate` from the ingester.
    pub rx: Receiver<StreamUpdate>,

    /// Shared in-memory map of partial events keyed by `PartialEventKey`,
    /// holding either-half events waiting for their pair. The watchdog holds a
    /// clone of this `Arc`.
    pub partials: Arc<DashMap<PartialEventKey, PartialEvent>>,

    /// Settlement program id (filter target for the decoder).
    pub settlement_program: Pubkey,

    /// SolFlow program id (filter target for the decoder).
    pub solflow_program: Pubkey,
}

impl Decoder {
    /// Construct a new decoder. The caller owns the channel capacity decision.
    pub fn new(
        persistence: Persistence,
        rx: Receiver<StreamUpdate>,
        partials: Arc<DashMap<PartialEventKey, PartialEvent>>,
        settlement_program: Pubkey,
        solflow_program: Pubkey,
    ) -> Self {
        Self {
            persistence,
            rx,
            partials,
            settlement_program,
            solflow_program,
        }
    }

    /// Main loop. Pulls `StreamUpdate` from the receiver, runs the decode
    /// pipeline, persists, and records partial events in the shared map for the
    /// watchdog to read.
    pub async fn run(&mut self) -> Result<(), PersistenceError> {
        unimplemented!()
    }
}
