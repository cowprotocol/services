//! The ingester drains the yellowstone gRPC stream as fast as it delivers,
//! pushes tagged updates into the channel, and advances the latest-chain-slot
//! counter on every slot-filter message. It performs no decoding.
//!
//! The stream it drains is an `AutoReconnect`-backed
//! [`GeyserStream`](yellowstone_grpc_client::GeyserStream) from
//! `yellowstone-grpc-client`: reconnects, backoff, and resume-from-checkpoint
//! are handled inside that stream and never surface
//! here. The ingester's [`Ingester::run`] loop therefore has no backoff of its
//! own; it returns when the stream ends (the wrapper gave up on an
//! unrecoverable error) or when the decoder hangs up.
//!
//! [`Ingester::serve`] is the production entrypoint — the "actual caller" —
//! that builds the subscription request, resumes from the persisted watermark,
//! opens the `GeyserStream`, and runs the drain loop. It expects the
//! [`GeyserGrpcClient`] it receives to have been built with a reconnect config
//! (via `set_reconnect_config`), otherwise the `AutoReconnect` wrapper won't
//! actually reconnect, and with HTTP/2 keepalive (`http2_keep_alive_interval`
//! / `keep_alive_while_idle`). The ingester does not answer server `Ping`
//! frames itself, so the transport keepalive is what holds an otherwise idle
//! connection open.

use {
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::StreamUpdate,
            errors::PersistenceError,
            slot::Slot,
            wire::{
                CommitmentLevel,
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
    /// writer. The `Arc` is taken from the caller so the finalization worker
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
    /// with the finalization worker, and reuse it across restarts. The caller
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
    // rather than `&self`, so the future borrows only those (both `Sync`) fields
    // across awaits. That keeps `run`'s future `Send` without requiring
    // `Ingester: Sync`. The `GeyserStream` field is `Send` but not `Sync`.
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
            UpdateOneof::Slot(slot) => Self::handle_slot(latest_chain_slot, slot).await,

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

    /// Consume a slot message: advance the in-memory chain-tip counter. Slot
    /// messages never enter the channel, so this always continues.
    async fn handle_slot(
        latest_chain_slot: &AtomicU64,
        slot: SubscribeUpdateSlot,
    ) -> ControlFlow<()> {
        latest_chain_slot.fetch_max(slot.slot, Ordering::Relaxed);
        ControlFlow::Continue(())
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
    /// The persisted watermark could not be read.
    #[error("failed to read the resume watermark: {0}")]
    CantReadWatermark(#[from] PersistenceError),
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
    /// Production entrypoint: build the subscription request, resume from the
    /// persisted watermark, open an `AutoReconnect`-backed `GeyserStream`, and
    /// run the drain loop.
    ///
    /// The initial `from_slot` is `watermark + 1`, or `None` on a cold start
    /// (the provider subscribes from the live tip). Reconnect `from_slot` is
    /// driven by the `AutoReconnect` wrapper's `BlockMeta` checkpoint, not this
    /// method.
    ///
    /// Returns `Ok(())` on a clean shutdown (the decoder dropped its receiver),
    /// or `Err(Error)` if setup failed or the stream ended terminally. The
    /// client is consumed and dropped with the ingester.
    ///
    /// `latest_chain_slot` is taken from the caller so the same `Arc` can be
    /// shared with the finalization worker and reused across restarts.
    pub async fn serve(
        mut client: GeyserGrpcClient,
        tx: Sender<StreamUpdate>,
        persistence: Persistence,
        latest_chain_slot: Arc<AtomicU64>,
        settlement_program: Pubkey,
        solflow_program: Pubkey,
    ) -> Result<(), Error> {
        let from_slot = persistence
            .read_watermark()
            .await?
            .map(|watermark| watermark + 1);
        let request = subscribe_request(settlement_program, solflow_program, from_slot);

        // The sink is the bidi request half: if kept, it can reconfigure the
        // subscription at runtime (add/remove a tracked program, change commitment,
        // narrow filters). Not used for this puprose at this time, but worth
        // considering in case our indexing requirements get more dynamic.
        let (_sink, stream) = client.subscribe_with_request(Some(request)).await?;

        let mut ingester = Ingester::new(stream, tx, latest_chain_slot);
        ingester.run().await
    }
}

/// Temporary compile-time proof that [`Ingester::serve`]'s future is `Send`.
///
/// Keep this only until a real `tokio::spawn(Ingester::serve(...))` call site
/// lands; the actual spawn is the better check. Delete this helper then.
#[allow(dead_code)]
fn assert_serve_future_is_send(
    client: GeyserGrpcClient,
    tx: Sender<StreamUpdate>,
    persistence: Persistence,
    latest_chain_slot: Arc<AtomicU64>,
    settlement_program: Pubkey,
    solflow_program: Pubkey,
) {
    fn is_send<F: Send>(_: F) {}
    is_send(Ingester::serve(
        client,
        tx,
        persistence,
        latest_chain_slot,
        settlement_program,
        solflow_program,
    ));
}

/// The wire-level filter shape: the two named transaction filters and the
/// `chain_tip` slot filter, multiplexed into a single subscription at
/// `confirmed` commitment. `from_slot` is the resume slot passed in by
/// [`Ingester::serve`] (watermark + 1, or `None` for the live tip).
///
/// The library auto-adds a `BlockMeta` + `slot` filter (under its
/// `__autoreconnect` key) so the `AutoReconnect` wrapper can checkpoint and
/// resume on reconnect; those messages are consumed inside the wrapper and
/// never reach the ingester.
///
/// TODO: source the exact subscriptions from a config file once this crate's
/// configuration module lands.
fn subscribe_request(
    settlement_program: Pubkey,
    solflow_program: Pubkey,
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
    SubscribeRequest {
        transactions: [
            (
                "settlement_txs".to_owned(),
                transactions(settlement_program),
            ),
            ("sol_flow_txs".to_owned(), transactions(solflow_program)),
        ]
        .into(),
        slots: [(
            "chain_tip".to_owned(),
            SubscribeRequestFilterSlots {
                // one message per slot at the subscription's commitment level
                filter_by_commitment: Some(true),
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
