//! Fan-out of orders as they arrive in the orderbook.
//!
//! Producers (currently the Postgres order listener) publish orders as soon as
//! they show up, consumers subscribe independently so work that would
//! otherwise happen on the auction cut's critical path (warming the banned
//! users cache, ...) can start early.

use {crate::domain::OrderUid, tokio::sync::broadcast, tokio_stream::wrappers::BroadcastStream};

/// How many arrivals a consumer may fall behind before it starts missing
/// orders. Consumers are best effort, so lagging is not fatal. In mainnet we're
/// (at the time of writing) running around ~1 order/second, so 16 should be
/// plenty space for new orders coming in.
const CAPACITY: usize = 16;

/// Publishing end of the order arrival fan-out.
#[derive(Clone)]
pub struct Notifier(broadcast::Sender<OrderUid>);

impl Notifier {
    pub fn new() -> Self {
        Self(broadcast::Sender::new(CAPACITY))
    }

    /// Publishes a newly arrived order. Arrivals published while nobody is
    /// subscribed are dropped.
    pub fn publish(&self, order: OrderUid) {
        if let Err(_) = self.0.send(order) {
            tracing::error!("failed to send order uid to subscribers");
        }
    }

    /// Stream of the orders arriving from now on, ending once all publishers
    /// are gone.
    ///
    /// A consumer that can't keep up is told how many orders it missed
    /// ([`BroadcastStreamRecvError::Lagged`]) instead of slowing down the
    /// publisher or its fellow consumers.
    pub fn subscribe(&self) -> BroadcastStream<OrderUid> {
        BroadcastStream::new(self.0.subscribe())
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        tokio_stream::{StreamExt, wrappers::errors::BroadcastStreamRecvError},
    };

    fn order(n: usize) -> OrderUid {
        let mut uid = [0u8; 56];
        uid[..size_of::<usize>()].copy_from_slice(&n.to_le_bytes());
        OrderUid(uid)
    }

    #[tokio::test]
    async fn every_subscriber_sees_every_arrival() {
        let arrivals = Notifier::new();
        let (mut first, mut second) = (arrivals.subscribe(), arrivals.subscribe());

        arrivals.publish(order(1));
        arrivals.publish(order(2));

        assert_eq!(first.next().await, Some(Ok(order(1))));
        assert_eq!(first.next().await, Some(Ok(order(2))));
        assert_eq!(second.next().await, Some(Ok(order(1))));
        assert_eq!(second.next().await, Some(Ok(order(2))));
    }

    #[tokio::test]
    async fn lagging_subscriber_is_told_what_it_missed() {
        let arrivals = Notifier::new();
        let mut subscription = arrivals.subscribe();

        for n in 0..=CAPACITY {
            arrivals.publish(order(n));
        }

        // The oldest arrival got dropped instead of stalling the publisher.
        assert_eq!(
            subscription.next().await,
            Some(Err(BroadcastStreamRecvError::Lagged(1)))
        );
        assert_eq!(subscription.next().await, Some(Ok(order(1))));
    }
}
