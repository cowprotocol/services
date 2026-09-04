//! The ingester drains the yellowstone gRPC stream as fast as it delivers,
//! pushes tagged updates into the channel, and advances the latest-chain-slot
//! counter on every confirmed slot message. It performs no decoding.
//!
//! The stream it drains is an `AutoReconnect`-backed
//! [`GeyserStream`](yellowstone_grpc_client::GeyserStream) from
//! `yellowstone-grpc-client`: reconnects and backoff are handled inside that
//! stream and never surface here, and a reconnect continues from the live
//! head. The ingester's [`Ingester::run`] loop therefore has no backoff of its
//! own; it returns when the stream ends (the wrapper gave up on an
//! unrecoverable error) or when the decoder hangs up.
//!
//! [`Ingester::serve`] is the production entrypoint — the "actual caller" —
//! that builds the subscription request, resumes past the last indexed slot,
//! opens the `GeyserStream`, and runs the drain loop. It expects the
//! [`GeyserGrpcClient`] it receives to have been built with a reconnect config
//! (via `set_reconnect_config`), otherwise the `AutoReconnect` wrapper won't
//! actually reconnect, and with HTTP/2 keepalive (`http2_keep_alive_interval`
//! / `keep_alive_while_idle`). The ingester does not answer server `Ping`
//! frames itself, so the transport keepalive is what holds an otherwise idle
//! connection open.

use {
    crate::{
        persistence::Postgres,
        types::{
            Signature,
            channel::StreamUpdate,
            errors::PersistenceError,
            slot::Slot,
            wire::{
                CommitmentLevel,
                SlotStatus,
                SubscribeRequest,
                SubscribeRequestFilterSlots,
                SubscribeRequestFilterTransactions,
                SubscribeUpdate,
                SubscribeUpdateSlot,
                SubscribeUpdateTransaction,
                UpdateOneof,
            },
        },
    },
    futures::stream::{Stream, StreamExt},
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        ops::ControlFlow,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    },
    tokio::sync::mpsc::{Sender, error::TrySendError},
    yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientError, GeyserStream},
    yellowstone_grpc_proto::tonic::Status,
};

/// Capacity of the channel from the ingester to the decoder.
pub const INGEST_TO_DECODER_CAPACITY: usize = 1024;

/// Ingester component.
///
/// Generic over the update `Stream` so unit tests can drive it with a mock.
/// Production wires this to an `AutoReconnect`-backed `GeyserStream` via
/// [`Ingester::serve`].
///
/// `Ping`/`Pong` frames are ignored: the library passes them through, but they
/// carry no data the ingester needs, and answering server pings is not part of
/// the drain path.
pub(crate) struct Ingester<S>
where
    S: Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send,
{
    /// The yellowstone update stream. Expected to be `AutoReconnect`-backed in
    /// production, so reconnects happen inside the stream and never surface to
    /// the drain loop.
    pub stream: S,

    /// Sends `StreamUpdate` to the decoder. Should be bounded to
    /// `INGEST_TO_DECODER_CAPACITY` entries.
    pub tx: Sender<StreamUpdate>,

    /// Latest chain slot seen on the slot filter. The ingester is the sole
    /// writer. The `Arc` is taken from the caller so other components
    /// can share it as a read handle once it is wired up; it doesn't read it
    /// yet. Cold start is zero (`AtomicU64::default`).
    pub latest_chain_slot: Arc<AtomicU64>,
}

impl<S> Ingester<S>
where
    S: Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send,
{
    /// Construct a new ingester over an already-open update stream. The caller
    /// supplies `latest_chain_slot` so it can share the same `Arc<AtomicU64>`
    /// with other components, and reuse it across restarts. The caller
    /// also owns building the stream, the
    /// subscription request, the resume slot, and the reconnect policy that
    /// come with it. Production wiring lives in [`Ingester::serve`].
    pub fn new(stream: S, tx: Sender<StreamUpdate>, latest_chain_slot: Arc<AtomicU64>) -> Self {
        Self {
            stream,
            tx,
            latest_chain_slot,
        }
    }

    /// Drain the update stream until it ends or the decoder hangs up.
    ///
    /// Recoverable stream errors never reach this loop: the `AutoReconnect`
    /// wrapper handles them internally. Returns `Ok(())` when the decoder
    /// dropped its receiver (clean shutdown), or [`Err(Error)`] when the stream
    /// ended terminally (the wrapper gave up on an unrecoverable error, or the
    /// stream closed).
    pub async fn run(&mut self) -> Result<(), Error> {
        while let Some(update) = self.stream.next().await {
            match update {
                Ok(update) => {
                    if Self::handle_update(&self.tx, &self.latest_chain_slot, update)
                        .await
                        .is_break()
                    {
                        tracing::info!("decoder channel closed; ingester stopping");
                        return Ok(());
                    }
                }
                Err(status) => {
                    tracing::warn!(%status, "yellowstone stream error; ingester stopping");
                    return Err(Error::Stream(status));
                }
            }
        }
        tracing::info!("yellowstone stream ended; ingester stopping");
        Err(Error::StreamEnded)
    }

    /// Dispatch one wire message. Breaks when the decoder is gone.
    //
    // Associated function taking the channel and chain-tip counter by reference
    // rather than `&self`, so the future borrows only those (both `Sync`)
    // fields across awaits. That keeps `run`'s future `Send` without
    // requiring `Ingester: Sync`. The `GeyserStream` field is `Send` but
    // not `Sync`.
    async fn handle_update(
        tx: &Sender<StreamUpdate>,
        latest_chain_slot: &AtomicU64,
        update: SubscribeUpdate,
    ) -> ControlFlow<()> {
        let Some(update) = update.update_oneof else {
            tracing::warn!(
                latest_chain_slot = latest_chain_slot.load(Ordering::Relaxed),
                "update without a payload"
            );
            return ControlFlow::Continue(());
        };
        match update {
            UpdateOneof::Transaction(tx_msg) => Self::handle_transaction(tx, tx_msg).await,
            UpdateOneof::Slot(slot) => Self::handle_slot(tx, latest_chain_slot, slot).await,

            // Ping/Pong frames carry no data the ingester needs; the library passes them through,
            // and we drop them here.
            UpdateOneof::Ping(_) | UpdateOneof::Pong(_) => ControlFlow::Continue(()),

            // Not part of our subscription; irrelevant to the ingester even if the provider sends
            // them.
            UpdateOneof::Account(_)
            | UpdateOneof::TransactionStatus(_)
            | UpdateOneof::Block(_)
            | UpdateOneof::BlockMeta(_)
            | UpdateOneof::Entry(_) => ControlFlow::Continue(()),
        }
    }

    /// Forward a transaction update to the decoder, skipping frames without a
    /// body or with a malformed signature.
    #[tracing::instrument(skip_all, fields(slot = tx_msg.slot))]
    async fn handle_transaction(
        tx: &Sender<StreamUpdate>,
        tx_msg: SubscribeUpdateTransaction,
    ) -> ControlFlow<()> {
        let Some(inner) = tx_msg.transaction else {
            tracing::warn!("transaction update without a body");
            return ControlFlow::Continue(());
        };
        let Ok(signature) = Signature::try_from(inner.signature.as_slice()) else {
            tracing::warn!("transaction update with a malformed signature");
            return ControlFlow::Continue(());
        };
        Self::forward(
            tx,
            StreamUpdate::Tx {
                slot: Slot(tx_msg.slot),
                signature,
                inner: Box::new(inner),
            },
        )
        .await
    }

    /// Consume a slot message, routed by its status. A confirmed slot
    /// advances the in-memory chain-tip counter and lets the decoder flush a
    /// finished buffer. A finalized slot advances the finalized watermark.
    /// Every other status is dropped: flushing on a slot ahead of the
    /// transaction stream's commitment would declare slots complete whose
    /// transactions are still in flight.
    async fn handle_slot(
        tx: &Sender<StreamUpdate>,
        latest_chain_slot: &AtomicU64,
        slot: SubscribeUpdateSlot,
    ) -> ControlFlow<()> {
        match slot.status() {
            SlotStatus::SlotConfirmed => {
                latest_chain_slot.fetch_max(slot.slot, Ordering::Relaxed);
                Self::forward(
                    tx,
                    StreamUpdate::Slot {
                        slot: Slot(slot.slot),
                    },
                )
                .await
            }
            SlotStatus::SlotFinalized => {
                Self::forward(
                    tx,
                    StreamUpdate::Finalized {
                        slot: Slot(slot.slot),
                    },
                )
                .await
            }
            _ => ControlFlow::Continue(()),
        }
    }

    /// Push one update into the decoder channel. A full channel is the intended
    /// overload signal: warn once, then block until the decoder drains. Breaks
    /// when the decoder dropped its receiver.
    async fn forward(tx: &Sender<StreamUpdate>, update: StreamUpdate) -> ControlFlow<()> {
        match tx.try_send(update) {
            Ok(()) => ControlFlow::Continue(()),
            Err(TrySendError::Full(update)) => {
                // TODO: Rate-limit if sustained backpressure floods logs.
                tracing::warn!("decoder channel full; ingester blocked on backpressure");
                match tx.send(update).await {
                    Ok(()) => ControlFlow::Continue(()),
                    Err(_) => ControlFlow::Break(()),
                }
            }
            Err(TrySendError::Closed(_)) => ControlFlow::Break(()),
        }
    }
}

/// Why the ingester stopped.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The persisted last indexed slot could not be read.
    #[error("failed to read the last indexed slot: {0}")]
    CantReadLastIndexedSlot(#[from] PersistenceError),
    /// The yellowstone subscription could not be opened.
    #[error("failed to open the yellowstone subscription: {0}")]
    Subscribe(#[from] GeyserGrpcClientError),
    /// The stream returned a terminal gRPC error — the `AutoReconnect` wrapper
    /// gave up on an unrecoverable failure.
    #[error("yellowstone stream error: {0}")]
    Stream(#[from] Status),
    /// The stream ended without an error — the `AutoReconnect` wrapper stopped.
    #[error("yellowstone stream ended")]
    StreamEnded,
}

impl Ingester<GeyserStream> {
    /// Production entrypoint: build the subscription request, resume past
    /// the persisted last indexed slot, open an `AutoReconnect`-backed
    /// `GeyserStream`, and run the drain loop.
    ///
    /// The initial `from_slot` is `last_indexed_slot + 1`, or `None` on a cold
    /// start (the provider subscribes from the live tip). Reconnects inside
    /// the stream start from the live head, not from this slot.
    ///
    /// Returns `Ok(())` on a clean shutdown (the decoder dropped its receiver),
    /// or `Err(Error)` if setup failed or the stream ended terminally. The
    /// client is consumed and dropped with the ingester.
    ///
    /// `latest_chain_slot` is taken from the caller so the same `Arc` can be
    /// shared with other components and reused across restarts.
    pub async fn serve(
        mut client: GeyserGrpcClient,
        tx: Sender<StreamUpdate>,
        persistence: Postgres,
        latest_chain_slot: Arc<AtomicU64>,
        settlement_program: Pubkey,
        solflow_program: Option<Pubkey>,
        resume: Resume,
    ) -> Result<(), Error> {
        // The proto field is a bare slot number, and `from_slot` is inclusive,
        // so resume one past the last fully persisted slot.
        let from_slot = match resume {
            Resume::Watermark => persistence
                .last_indexed_slot()
                .await?
                .map(|last_indexed| u64::from(last_indexed) + 1),
            Resume::LiveTip => None,
            Resume::From(slot) => Some(slot),
        };
        let request = subscribe_request(settlement_program, solflow_program, from_slot);

        // The sink is the bidi request half: if kept, it can reconfigure the
        // subscription at runtime (add/remove a tracked program, change
        // commitment, narrow filters). Not used for this puprose at
        // this time, but worth considering in case our indexing
        // requirements get more dynamic.
        let (_sink, stream) = client.subscribe_with_request(Some(request)).await?;

        let mut ingester = Ingester::new(stream, tx, latest_chain_slot);
        ingester.run().await
    }
}

/// Filter labels in the subscription request. The server echoes them on
/// matching updates, nothing routes on them today.
const SETTLEMENT_FILTER: &str = "settlement_txs";
const SOLFLOW_FILTER: &str = "sol_flow_txs";
const SLOT_FILTER: &str = "slot_statuses";

/// Where a fresh subscription starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resume {
    /// One past the persisted last indexed slot.
    Watermark,
    /// The provider's live tip, accepting a gap. The fallback when the
    /// watermark is older than the provider's replay window.
    LiveTip,
    /// A caller-chosen slot, still bounded by the provider's replay window.
    From(u64),
}

/// The wire-level filter shape: the two named transaction filters and the
/// slot-status filter, multiplexed into a single subscription at
/// `confirmed` commitment. `from_slot` is the resume slot passed in by
/// [`Ingester::serve`] (`last_indexed_slot + 1`, or `None` for the live tip).
///
/// The library auto-adds a `BlockMeta` + `slot` filter under its
/// `__autoreconnect` key. Those messages are consumed inside the wrapper and
/// never reach the ingester.
fn subscribe_request(
    settlement_program: Pubkey,
    solflow_program: Option<Pubkey>,
    from_slot: Option<u64>,
) -> SubscribeRequest {
    // `failed: None` includes failed transactions: the failure itself is the
    // on-chain signal downstream consumers read.
    let transactions = |program: Pubkey| SubscribeRequestFilterTransactions {
        vote: Some(false),
        failed: None,
        account_include: vec![program.to_string()],
        ..Default::default()
    };
    let mut filters = HashMap::from([(
        SETTLEMENT_FILTER.to_owned(),
        transactions(settlement_program),
    )]);
    if let Some(solflow) = solflow_program {
        filters.insert(SOLFLOW_FILTER.to_owned(), transactions(solflow));
    }
    SubscribeRequest {
        transactions: filters,
        slots: [(
            SLOT_FILTER.to_owned(),
            SubscribeRequestFilterSlots {
                // Every status transition, so finalized slots arrive next to
                // confirmed ones. The ingester routes the two it needs and
                // drops the rest.
                filter_by_commitment: Some(false),
                ..Default::default()
            },
        )]
        .into(),
        commitment: Some(CommitmentLevel::Confirmed as i32),
        from_slot,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
