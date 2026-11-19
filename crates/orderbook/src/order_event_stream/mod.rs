//! Streams the life cycle events of an order to API clients. One database
//! connection per process listens for the events, so that an open stream
//! needs no connection of its own.

mod listener;

use {
    crate::{
        database::Postgres,
        dto::order_event::{Label, OrderEvent},
    },
    database::orders::Progress,
    futures::Stream,
    model::order::OrderUid,
    sqlx::PgPool,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    },
    tokio::sync::{OwnedSemaphorePermit, Semaphore, broadcast},
};

/// Maximum number of streams open at the same time.
const MAX_STREAMS: usize = 10_000;

/// Number of events a stream may fall behind before it gets closed.
const MAX_LAG: usize = 16;

/// How long after its `valid_to` an order can still get an event: a trade in
/// one of the last valid blocks is observed some time later.
const EXPIRY_GRACE: Duration = Duration::from_secs(300);

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum SubscribeError {
    #[error("too many open streams")]
    TooManyStreams,
    #[error("the stream is shutting down")]
    Closed,
}

/// Hands every order event to the subscriptions of that order.
pub struct OrderEventStream {
    state: Mutex<State>,
    stream_permits: Arc<Semaphore>,
    /// Only tells if an order is finished, so it can be a replica.
    database: Postgres,
}

#[derive(Default)]
struct State {
    subscribers: HashMap<OrderUid, broadcast::Sender<OrderEvent>>,
    closed: bool,
}

impl OrderEventStream {
    /// Starts to listen for order events on `primary`. A replica does not
    /// deliver notifications.
    pub fn spawn(primary: PgPool, database: Postgres) -> Arc<Self> {
        let order_event_stream = Self::new(MAX_STREAMS, database);
        listener::spawn_notification_listener(order_event_stream.clone(), primary);
        order_event_stream
    }

    fn new(max_streams: usize, database: Postgres) -> Arc<Self> {
        Arc::new(Self {
            state: Default::default(),
            stream_permits: Arc::new(Semaphore::new(max_streams)),
            database,
        })
    }

    /// Opens the stream of the events an order gets from now on. `None` if
    /// the order is finished and gets no more events.
    pub async fn open(
        self: &Arc<Self>,
        uid: OrderUid,
    ) -> Result<Option<impl Stream<Item = OrderEvent> + Send + use<>>, SubscribeError> {
        // Subscribed before the read: a final event that the read does not
        // see yet then arrives in the subscription.
        let subscription = self.subscribe(uid)?;
        let progress = self
            .database
            .order_progress(&uid)
            .await
            .inspect_err(|err| tracing::warn!(?err, %uid, "failed to read the order progress"))
            .ok()
            .flatten();

        let (_, _, valid_to) = uid.parts();
        let time_left = time_until_last_event(valid_to, model::time::now_in_epoch_seconds());
        if time_left.is_zero() || progress.is_some_and(is_finished) {
            return Ok(None);
        }
        // An order that is not known counts as partially fillable, which
        // keeps its stream open after a trade.
        let fill_or_kill = progress.is_some_and(|progress| !progress.partially_fillable);
        Ok(Some(events_until_finished(
            subscription,
            fill_or_kill,
            time_left,
        )))
    }

    fn publish(&self, event: OrderEvent) {
        Metrics::get().events.inc();
        let state = self.state.lock().unwrap();
        if let Some(sender) = state.subscribers.get(&event.uid) {
            // Fails only if all receivers are gone, which their
            // subscriptions clean up.
            let _ = sender.send(event);
        }
    }

    /// Starts a subscription to the events of one order. It only receives
    /// events that happen from now on.
    fn subscribe(self: &Arc<Self>, uid: OrderUid) -> Result<Subscription, SubscribeError> {
        let permit = self
            .stream_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| SubscribeError::TooManyStreams)?;

        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(SubscribeError::Closed);
        }
        let receiver = state
            .subscribers
            .entry(uid)
            .or_insert_with(|| broadcast::channel(MAX_LAG).0)
            .subscribe();
        drop(state);

        Metrics::get().open_streams.inc();
        Ok(Subscription {
            order_event_stream: self.clone(),
            uid,
            receiver: Some(receiver),
            _permit: permit,
        })
    }

    /// Ends all subscriptions. Necessary after notifications were lost: the
    /// clients then reconnect and load the current state of their order.
    fn end_subscriptions(&self) {
        self.state.lock().unwrap().subscribers.clear();
    }

    /// Ends all subscriptions and refuses new ones.
    pub fn close(&self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        state.subscribers.clear();
    }
}

/// Whether the order gets no more events. A partially fillable order can
/// trade again, so only its cancellation and its expiry finish it.
fn is_finished(progress: Progress) -> bool {
    progress.cancelled || (progress.traded && !progress.partially_fillable)
}

/// Time until an order with this `valid_to` can no longer get an event. Both
/// times are in seconds since the Unix epoch.
fn time_until_last_event(valid_to: u32, now: u32) -> Duration {
    (Duration::from_secs(valid_to.into()) + EXPIRY_GRACE)
        .saturating_sub(Duration::from_secs(now.into()))
}

/// The events of a subscription up to the event that finishes the order, or
/// until `time_left` has passed.
fn events_until_finished(
    mut subscription: Subscription,
    fill_or_kill: bool,
    time_left: Duration,
) -> impl Stream<Item = OrderEvent> + Send {
    async_stream::stream! {
        let expiry = tokio::time::sleep(time_left);
        tokio::pin!(expiry);
        loop {
            let event = tokio::select! {
                event = subscription.next_event() => event,
                _ = &mut expiry => None,
            };
            let Some(event) = event else {
                break;
            };
            let label = event.label;
            yield event;
            if label == Label::Cancelled || (label == Label::Traded && fill_or_kill) {
                break;
            }
        }
    }
}

/// Receives the events of one order until it is dropped.
struct Subscription {
    order_event_stream: Arc<OrderEventStream>,
    uid: OrderUid,
    /// Only `None` while dropping.
    receiver: Option<broadcast::Receiver<OrderEvent>>,
    _permit: OwnedSemaphorePermit,
}

impl Subscription {
    /// The next event, or `None` when the subscription ended: the stream
    /// ended it or the receiver fell too far behind.
    async fn next_event(&mut self) -> Option<OrderEvent> {
        match self.receiver.as_mut()?.recv().await {
            Ok(event) => Some(event),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                Metrics::get().lagged_streams.inc();
                None
            }
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut state = self.order_event_stream.state.lock().unwrap();
        self.receiver = None;
        // After the subscriptions were ended, the entry can belong to newer
        // ones.
        if state
            .subscribers
            .get(&self.uid)
            .is_some_and(|sender| sender.receiver_count() == 0)
        {
            state.subscribers.remove(&self.uid);
        }
        Metrics::get().open_streams.dec();
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "order_event_stream")]
struct Metrics {
    /// Number of currently open order event streams.
    open_streams: prometheus::IntGauge,

    /// Number of order events received from the database.
    events: prometheus::IntCounter,

    /// Number of streams closed because the client read too slowly.
    lagged_streams: prometheus::IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        chrono::{TimeZone, Utc},
        futures::StreamExt,
    };

    fn order_event_stream(max_streams: usize) -> Arc<OrderEventStream> {
        // The pool is lazy and these tests never use it.
        let database = Postgres::try_new("postgresql://", Default::default()).unwrap();
        OrderEventStream::new(max_streams, database)
    }

    fn event(uid: u32, label: Label) -> OrderEvent {
        OrderEvent {
            uid: OrderUid::from_integer(uid),
            timestamp: Utc.timestamp_opt(0, 0).unwrap(),
            label,
            reason: None,
        }
    }

    /// The labels a stream sends for the published `labels` before it ends.
    async fn labels_until_finished(labels: &[Label], fill_or_kill: bool) -> Vec<Label> {
        let order_event_stream = order_event_stream(10);
        let subscription = order_event_stream
            .subscribe(OrderUid::from_integer(1))
            .unwrap();
        for label in labels {
            order_event_stream.publish(event(1, *label));
        }
        // Ends the stream if the labels did not.
        order_event_stream.end_subscriptions();

        events_until_finished(subscription, fill_or_kill, Duration::from_secs(60))
            .map(|event| event.label)
            .collect()
            .await
    }

    #[tokio::test]
    async fn delivers_only_the_events_of_the_subscribed_order() {
        let order_event_stream = order_event_stream(10);
        let mut first = order_event_stream
            .subscribe(OrderUid::from_integer(1))
            .unwrap();
        let mut second = order_event_stream
            .subscribe(OrderUid::from_integer(1))
            .unwrap();

        order_event_stream.publish(event(2, Label::Created));
        order_event_stream.publish(event(1, Label::Ready));
        order_event_stream.publish(event(1, Label::Traded));

        for subscription in [&mut first, &mut second] {
            assert_eq!(
                subscription.next_event().await,
                Some(event(1, Label::Ready))
            );
            assert_eq!(
                subscription.next_event().await,
                Some(event(1, Label::Traded))
            );
        }
    }

    #[tokio::test]
    async fn stream_ends_after_the_event_that_finishes_the_order() {
        use Label::*;
        let published = [Ready, Traded, Ready, Cancelled, Ready];

        assert_eq!(
            labels_until_finished(&published, true).await,
            [Ready, Traded]
        );
        assert_eq!(
            labels_until_finished(&published, false).await,
            [Ready, Traded, Ready, Cancelled]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stream_ends_when_the_order_expires() {
        let order_event_stream = order_event_stream(10);
        let subscription = order_event_stream
            .subscribe(OrderUid::from_integer(1))
            .unwrap();
        let events = events_until_finished(subscription, true, Duration::from_secs(60));
        futures::pin_mut!(events);

        order_event_stream.publish(event(1, Label::Ready));
        assert_eq!(events.next().await, Some(event(1, Label::Ready)));

        let started = tokio::time::Instant::now();
        assert_eq!(events.next().await, None);
        assert_eq!(started.elapsed(), Duration::from_secs(60));
    }

    #[test]
    fn order_can_get_events_until_the_grace_period_ends() {
        let grace = u32::try_from(EXPIRY_GRACE.as_secs()).unwrap();
        assert_eq!(
            time_until_last_event(1_000, 400),
            Duration::from_secs(600) + EXPIRY_GRACE
        );
        assert_eq!(time_until_last_event(1_000, 1_000), EXPIRY_GRACE);
        assert_eq!(time_until_last_event(1_000, 1_000 + grace), Duration::ZERO);
        assert_eq!(time_until_last_event(1_000, u32::MAX), Duration::ZERO);
        assert!(!time_until_last_event(u32::MAX, 1_000).is_zero());
    }

    #[test]
    fn only_a_cancellation_finishes_a_partially_fillable_order() {
        let progress = |partially_fillable, cancelled, traded| Progress {
            partially_fillable,
            cancelled,
            traded,
        };
        assert!(!is_finished(progress(false, false, false)));
        assert!(is_finished(progress(false, false, true)));
        assert!(is_finished(progress(false, true, false)));
        assert!(!is_finished(progress(true, false, true)));
        assert!(is_finished(progress(true, true, true)));
    }

    #[tokio::test]
    async fn slow_subscription_ends() {
        let order_event_stream = order_event_stream(10);
        let mut subscription = order_event_stream
            .subscribe(OrderUid::from_integer(1))
            .unwrap();

        for _ in 0..=MAX_LAG {
            order_event_stream.publish(event(1, Label::Ready));
        }

        assert_eq!(subscription.next_event().await, None);
    }

    #[tokio::test]
    async fn ended_subscription_does_not_remove_its_successor() {
        let order_event_stream = order_event_stream(10);
        let uid = OrderUid::from_integer(1);
        let mut ended = order_event_stream.subscribe(uid).unwrap();

        order_event_stream.end_subscriptions();
        assert_eq!(ended.next_event().await, None);

        let mut successor = order_event_stream.subscribe(uid).unwrap();
        drop(ended);
        order_event_stream.publish(event(1, Label::Ready));
        assert_eq!(successor.next_event().await, Some(event(1, Label::Ready)));
    }

    #[tokio::test]
    async fn close_refuses_new_subscriptions() {
        let order_event_stream = order_event_stream(10);
        let uid = OrderUid::from_integer(1);
        let mut subscription = order_event_stream.subscribe(uid).unwrap();

        order_event_stream.close();

        assert_eq!(subscription.next_event().await, None);
        assert_eq!(
            order_event_stream.subscribe(uid).err(),
            Some(SubscribeError::Closed)
        );
    }

    #[tokio::test]
    async fn limits_open_streams_and_frees_their_channels() {
        let order_event_stream = order_event_stream(1);
        let uid = OrderUid::from_integer(1);
        let subscription = order_event_stream.subscribe(uid).unwrap();
        assert_eq!(
            order_event_stream.subscribe(uid).err(),
            Some(SubscribeError::TooManyStreams)
        );

        drop(subscription);

        assert!(
            order_event_stream
                .state
                .lock()
                .unwrap()
                .subscribers
                .is_empty()
        );
        assert!(order_event_stream.subscribe(uid).is_ok());
    }
}
