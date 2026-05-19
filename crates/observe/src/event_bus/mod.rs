//! Implements a simple globally available way to publish events to an event
//! bus. Under the hood it's using a RabbitMQ stream. Because the RabbitMQ crate
//! internally uses a bounded channel which makes every send an async operation
//! we additionally buffer messages in an unbounded channel to also allow
//! publishing events from synchronous contexts.
//! The architecture looks roughly like this:
//! cow service: business_logic --message--> event_bus_channel
//! event_bus_background_task: event_bus_channel --message--> rabbitmq_channel
//! rabbitmq_background_task: rabbitmq_channel --message--> rabbitmq_service

use {
    chrono::Utc,
    rabbitmq_stream_client::{
        Environment,
        NoDedup,
        Producer,
        types::{ByteCapacity, Message},
    },
    serde::Serialize,
    std::future::Future,
    tokio::sync::{
        OnceCell,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
};

/// Channel to buffer emitted events until we have enough to send a bunch of
/// them at once.
static EVENT_QUEUE: OnceCell<UnboundedSender<Message>> = OnceCell::const_new();

/// Configuration options describing to which RabbitMQ instance to connect to.
pub struct ClientConfig {
    host: String,
    port: u16,
}

/// Configuration options describing which RabbitMQ channel to publish messages
/// in.
pub struct ChannelConfig {
    name: String,
    size: ByteCapacity,
    /// Number of messages to buffer in-memory before actually sending them to
    /// the event bus service running in a different process.
    batch_size: usize,
}

pub struct Config {
    client: ClientConfig,
    channel: ChannelConfig,
}

/// Initializes the event bus and panics if it fails.
pub async fn init(config: Config, shutdown: impl Future<Output = ()> + Send + 'static) {
    EVENT_QUEUE
        .get_or_init(|| async move {
            let environment = Environment::builder()
                .host(&config.client.host)
                .port(config.client.port)
                .build()
                .await
                .expect("failed to connect to rabbitmq");

            environment
                .stream_creator()
                .max_length(config.channel.size)
                .create(&config.channel.name)
                .await
                .expect("failed to create channel");

            let producer = environment
                .producer()
                .batch_size(config.channel.batch_size)
                .build(&config.channel.name)
                .await
                .expect("failed to create producer");

            let (sender, receiver) = unbounded_channel();
            tokio::task::spawn(forward_messages_to_event_bus_client(
                receiver,
                producer,
                config.channel.batch_size,
                shutdown,
            ));
            sender
        })
        .await;
}

async fn send_batch(producer: &Producer<NoDedup>, messages: Vec<Message>) {
    if let Err(err) = producer
        .batch_send(messages, |res| async move {
            match res {
                Ok(status) => tracing::trace!(?status, "messages confirmed"),
                Err(err) => tracing::error!(?err, "failed to send messages"),
            }
        })
        .await
    {
        tracing::error!(?err, "failed to enqueue messages");
    }
}

/// Buffers incoming messages and sends a batch once the target batch size has
/// been reached. When `shutdown` resolves, flushes any buffered messages and
/// switches to publishing each subsequent message immediately.
async fn forward_messages_to_event_bus_client(
    mut receiver: UnboundedReceiver<Message>,
    producer: Producer<NoDedup>,
    batch_size: usize,
    shutdown: impl Future<Output = ()> + Send + 'static,
) {
    let mut buffer = Vec::with_capacity(batch_size);
    tokio::pin!(shutdown);

    // Normal mode: buffer messages and send in batches.
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            maybe_message = receiver.recv() => {
                let Some(message) = maybe_message else {
                    tracing::debug!("event queue was closed");
                    return;
                };
                buffer.push(message);
                if buffer.len() >= batch_size {
                    let mut messages = Vec::with_capacity(batch_size);
                    std::mem::swap(&mut messages, &mut buffer);
                    send_batch(&producer, messages).await;
                }
            }
        }
    }

    // Shutdown mode: flush the buffer, then publish each subsequent message
    // immediately without waiting to fill a batch.
    if !buffer.is_empty() {
        send_batch(&producer, std::mem::take(&mut buffer)).await;
    }
    while let Some(message) = receiver.recv().await {
        send_batch(&producer, vec![message]).await;
    }
}

/// Enqueues the event to be sent to the event bus in a background task.
pub fn publish(name: impl Into<String>, data: impl Serialize) {
    let Some(queue) = EVENT_QUEUE.get() else {
        tracing::error!("event queue not yet initialized");
        return;
    };

    let body = match serde_json::to_vec(&data) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(?err, "failed to serialize event");
            return;
        }
    };

    let message = Message::builder()
        .properties()
        .subject(name)
        .creation_time(Utc::now())
        .content_type("application/json")
        .message_builder()
        .body(body)
        .build();

    if let Err(err) = queue.send(message) {
        tracing::error!(?err, "failed to enqueue message");
    }
}
