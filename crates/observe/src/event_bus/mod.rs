//! Implements a simple globally available way to publish events to an event
//! bus. Under the hood it's using NATS. To support publishing events from
//! synchronous contexts we use a channel as an in-memory buffer.
//! Whenever a message gets posted to this channel a background task wakes
//! up and forwards it to the NATS service running in a different process.
//! Messages always get serialized as JSON so you can publish anything that
//! can be serialized to JSON as well.
use {
    crate::config::EventBusConfig,
    async_nats::jetstream::Context as JetstreamClient,
    bytes::Bytes,
    event_bus_dto::{Envelope, Event},
    futures::stream::{FuturesUnordered, StreamExt},
    serde::Serialize,
    tokio::sync::{
        OnceCell,
        mpsc::{Receiver, Sender, channel},
    },
};

struct EventBusConnector {
    /// Channel to decouple issuing events from actually sending them to the
    /// event bus service.
    message_queue: Sender<Message>,
    /// Subject prefix to disambiguate messages in globally shared event bus
    /// service.
    subject_prefix: String,
}

struct Message {
    subject: String,
    data: Bytes,
}

/// Singleton event bus connection to allow publishing events
/// conveniently from everywhere.
static BUS: OnceCell<EventBusConnector> = OnceCell::const_new();

/// Initializes the event bus. Connection failures are logged but do not
/// abort startup: the event bus is purely observational, so a misconfigured
/// or unreachable NATS must not take the binary down. When init fails the
/// global `BUS` stays uninitialized and [`publish`] becomes a no-op.
///
/// Safe to call multiple times: once a previous call has succeeded the
/// subsequent ones short-circuit. A failed call leaves the bus uninitialized
/// so the next call gets another chance.
pub async fn init(config: EventBusConfig) {
    if BUS.initialized() {
        return;
    }
    let result = BUS
        .get_or_try_init(|| async { connect(&config).await })
        .await;
    match result {
        Ok(_) => {
            tracing::info!(
                channel = %config.stream_name,
                chain_id = config.chain_id,
                "event bus connected",
            );
        }
        Err(err) => {
            tracing::error!(
                ?err,
                url = %config.url,
                channel = %config.stream_name,
                "failed to initialize event bus; events will be dropped",
            );
        }
    }
}

async fn connect(config: &EventBusConfig) -> Result<EventBusConnector, async_nats::Error> {
    // We prefix every subject with `event` so consumers can subscribe to all
    // events (e.g. `event.>`) without also seeing NATS internal events. The
    // trailing dot is significant: see [`publish`] for how it's concatenated
    // with the per-event subject suffix.
    let subject_prefix = format!("event.{}.", config.chain_id);

    let client = async_nats::connect(config.url.as_str()).await?;
    let jetstream = async_nats::jetstream::new(client);
    // Make sure the stream exists up-front; otherwise every publish would fail
    // server-side and we'd only find out at runtime.
    jetstream.get_stream(&config.stream_name).await?;

    // JetStream publish completes in two stages: the call to `publish()`
    // returns once the client has buffered the message, the returned
    // PubAck future resolves once the server has stored it.
    let (message_tx, message_rx) = channel(EVENT_BUS_SIZE);
    let (ack_tx, ack_rx) = channel(EVENT_BUS_SIZE);
    tokio::task::spawn(publish_messages(message_rx, jetstream, ack_tx));
    tokio::task::spawn(await_acks(ack_rx));

    Ok(EventBusConnector {
        message_queue: message_tx,
        subject_prefix,
    })
}

const EVENT_BUS_SIZE: usize = 1_000;

/// In-flight handle returned by JetStream's `publish` call, passed from the
/// publisher task to the ack-handling task.
type PendingAck = (String, async_nats::jetstream::context::PublishAckFuture);

/// Reads messages off the in-memory queue and hands the publish ack future
/// off to [`await_acks`].
async fn publish_messages(
    mut messages: Receiver<Message>,
    client: JetstreamClient,
    acks: Sender<PendingAck>,
) {
    while let Some(message) = messages.recv().await {
        let subject = message.subject;
        let ack_fut = match client.publish(subject.clone(), message.data).await {
            Ok(fut) => fut,
            Err(err) => {
                tracing::warn!(?err, %subject, "failed to enqueue event with NATS client");
                record_dropped(DropReason::Publish);
                continue;
            }
        };
        if acks.send((subject, ack_fut)).await.is_err() {
            tracing::warn!("ack task was shut down; returning");
            return;
        }
    }
}

/// Awaits JetStream publish acks concurrently and logs any failures. Runs
/// until the publisher task drops its sender and all in-flight acks have
/// resolved, so server-side rejections are still observed during shutdown.
async fn await_acks(mut acks: Receiver<PendingAck>) {
    let mut pending = FuturesUnordered::new();
    loop {
        tokio::select! {
            biased;
            // Drain pending acks alongside new ones so failures are logged
            // promptly and the set doesn't grow without bound.
            Some(()) = pending.next(), if !pending.is_empty() => {}
            next = acks.recv() => {
                let Some((subject, ack_fut)) = next else { break };
                pending.push(log_ack(subject, ack_fut));
            }
        }
    }
    // Only reached on publisher panic (steady state keeps both tasks alive).
    // Drain pending futures so `log_ack` still runs for any in-flight publish
    // — otherwise dropping them unpolled silently loses the log + metric.
    while pending.next().await.is_some() {}
}

async fn log_ack(subject: String, ack_fut: async_nats::jetstream::context::PublishAckFuture) {
    if let Err(err) = ack_fut.await {
        tracing::warn!(?err, %subject, "NATS did not acknowledge event");
        record_dropped(DropReason::Ack);
    }
}

/// Enqueues the event to be sent to the event bus in a background task.
///
/// For ad-hoc events, use [`publish`] instead.
pub fn publish_event<E: Event>(event: E) {
    publish(E::SUBJECT, event);
}

/// Enqueues the event to be sent to the event bus in a background task.
pub fn publish(subject: &str, data: impl Serialize) {
    let Some(bus) = BUS.get() else {
        tracing::trace!("attempting to publish events without initializing the event bus");
        return;
    };

    let envelope = Envelope::new(
        crate::tracing::distributed::request_id::from_current_span(),
        data,
    );
    let body = match serde_json::to_vec(&envelope) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(?err, "failed to serialize event");
            record_dropped(DropReason::Serialize);
            return;
        }
    };

    let message = Message {
        subject: format!("{}{}", bus.subject_prefix, subject),
        data: body.into(),
    };

    if let Err(err) = bus.message_queue.try_send(message) {
        tracing::error!(?err, "failed to enqueue message");
        record_dropped(DropReason::ChannelFull);
    }
}

/// Why an event was not delivered to the event bus. Used as a Prometheus
/// label so the failure modes can be alerted on independently.
#[derive(Copy, Clone, Debug)]
enum DropReason {
    /// In-memory queue between [`publish`] and the background forwarder was
    /// saturated.
    ChannelFull,
    /// The payload could not be encoded as JSON.
    Serialize,
    /// The NATS client rejected the publish locally (e.g. disconnected).
    Publish,
    /// JetStream did not acknowledge the publish.
    Ack,
}

impl DropReason {
    fn as_label(self) -> &'static str {
        match self {
            DropReason::ChannelFull => "channel_full",
            DropReason::Serialize => "serialize",
            DropReason::Publish => "publish",
            DropReason::Ack => "ack",
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "event_bus")]
struct Metrics {
    /// Events that were not delivered to the event bus, by failure mode.
    /// See [`DropReason`] for the meaning of each label value.
    #[metric(labels("reason"))]
    dropped_events: prometheus::IntCounterVec,
}

fn record_dropped(reason: DropReason) {
    let Ok(metrics) = Metrics::instance(crate::metrics::get_storage_registry()) else {
        return;
    };
    metrics
        .dropped_events
        .with_label_values(&[reason.as_label()])
        .inc();
}
