//! Implements a simple globally available way to publish events to an event
//! bus. Under the hood it's using NATS. To support publishing events from
//! synchronous contexts we use an unbounded channel as an in-memory buffer.
//! Whenever a message gets posted to this channel a background task wakes
//! up and forwards it to the NATS service running in a different process.
//! Messages always get serialized as JSON so you can publish anything that
//! can be serialized to JSON as well.
use {
    crate::config::EventBusConfig,
    async_nats::jetstream::Context as JetstreamClient,
    bytes::Bytes,
    chrono::Utc,
    serde::Serialize,
    serde_json::json,
    tokio::sync::{
        OnceCell,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
};

struct EventBusConnector {
    /// Unbounded channel to allow emitting events from synchrounous
    /// contexts.
    message_queue: UnboundedSender<Message>,
    /// Subject prefix to disambiguate messages in globally shared event bus
    /// service.
    subject_prefix: String,
}

struct Message {
    subject: String,
    data: Bytes,
}

/// Singleton event bus connection to allow publishing events
/// conventiently from everywhere.
static BUS: OnceCell<EventBusConnector> = OnceCell::const_new();

/// Initializes the event bus and panics if it fails.
pub async fn init(config: EventBusConfig) {
    BUS.get_or_init(|| async move {
        let client = async_nats::connect(config.url.as_str())
            .await
            .expect("failed to connect to NATS service");
        let jetstream = async_nats::jetstream::new(client);
        let mut stream = jetstream
            .get_stream(&config.channel)
            .await
            .expect("could not connect to jetstream");
        let info = stream.info().await.expect("failed to fetch stream info");
        tracing::debug!(?info, "connected to jetstream");

        let (sender, receiver) = unbounded_channel();
        tokio::task::spawn(forward_messages_to_event_bus_client(receiver, jetstream));
        EventBusConnector {
            message_queue: sender,
            // we prefix every subject with `event` to allow consumers to easily
            // subscribe to all events without also seeing NATS internal events
            subject_prefix: format!("event.{}.", config.chain_id),
        }
    })
    .await;
}

/// Monitors a message queue and forwards all messages to the event bus
/// service.
async fn forward_messages_to_event_bus_client(
    mut receiver: UnboundedReceiver<Message>,
    client: JetstreamClient,
) {
    while let Some(message) = receiver.recv().await {
        match client.publish(message.subject, message.data).await {
            Err(err) => {
                tracing::debug!(?err, "failed to publish event");
            }
            Ok(_fut) => {
                // let's assume the message arrived for now
            }
        }
    }
}

/// Enqueues the event to be sent to the event bus in a background task.
pub fn publish(subject: &str, data: impl Serialize) {
    let Some(bus) = BUS.get() else {
        return;
    };

    let mut message = json!({
        "version": "v1",
        "timestamp": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "body": data,
    });
    if let Some(id) = crate::tracing::distributed::request_id::from_current_span() {
        message["requestId"] = id.into();
    }
    let body = match serde_json::to_vec(&message) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(?err, "failed to serialize event");
            return;
        }
    };

    let message = Message {
        subject: format!("{}{}", bus.subject_prefix, subject),
        data: body.into(),
    };

    if let Err(err) = bus.message_queue.send(message) {
        tracing::error!(?err, "failed to enqueue message");
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[ignore]
    #[tokio::test]
    async fn send_messages() {
        crate::tracing::init::initialize(&crate::Config {
            env_filter: "warn,observe=debug".to_string(),
            stderr_threshold: None,
            use_json_format: false,
            tracing: None,
        });
        init(EventBusConfig {
            url: "localhost:4222".parse().unwrap(),
            channel: "main".to_string(),
            chain_id: 1,
        })
        .await;

        for _ in 0..1000 {
            publish(
                "name",
                json!({
                    "estimator": "baseline",
                    "outAmount": 1234,
                }),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
