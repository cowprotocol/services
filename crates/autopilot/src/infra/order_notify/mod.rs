mod banned;
mod run_loop;

use {
    self::run_loop::RunLoopWaker,
    crate::{domain::OrderUid, infra::order_notify::banned::CachePrewarmer},
    futures::{channel::mpsc, future::join_all},
    order_validation::banned::Users,
    sqlx::PgPool,
    std::{sync::Arc, time::Duration},
};

/// A system interested in every order arriving in the orderbook.
///
/// Notifications are best effort: orders arriving while the database
/// connection is down are never seen, so whatever a listener does must be
/// recoverable at auction cut time.
#[async_trait::async_trait]
pub trait Listener: Send + Sync {
    async fn on_new_order(&self, order: OrderUid);
}

/// Notifies the registered systems of orders arriving in the database.
pub struct Notifier {
    listeners: Vec<Box<dyn Listener>>,
}

impl Notifier {
    pub fn new(
        banned_users: Arc<Users>,
        run_loop_new_order_listener: mpsc::UnboundedSender<OrderUid>,
    ) -> Self {
        Self {
            listeners: vec![
                Box::new(RunLoopWaker(run_loop_new_order_listener)),
                Box::new(CachePrewarmer(banned_users)),
            ],
        }
    }

    /// Spawns a background task that listens for new order notifications from
    /// PostgreSQL and forwards every arriving order to the registered systems.
    pub fn spawn(self, pool: PgPool) {
        tokio::spawn(async move {
            loop {
                let mut listener = match sqlx::postgres::PgListener::connect_with(&pool).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        tracing::error!(?err, "failed to create PostgreSQL listener");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                if let Err(err) = listener.listen("new_order").await {
                    tracing::error!(?err, "failed to listen on 'new_order' channel");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                tracing::info!("connected to PostgreSQL for order notifications");

                loop {
                    match listener.recv().await {
                        Ok(notification) => self.dispatch(notification.payload()).await,
                        Err(err) => {
                            tracing::error!(?err, "error receiving notification from postgres");
                            break;
                        }
                    }
                }
            }
        });
    }

    async fn dispatch(&self, payload: &str) {
        tracing::debug!(payload, "received order notification from postgres");
        let Some(order) = order_uid_from_notification(payload) else {
            tracing::warn!(payload, "malformed order notification payload");
            return;
        };
        join_all(self.listeners.iter().map(|listener| async move {
            listener.on_new_order(order).await;
        }))
        .await;
    }
}

/// Parses the payload of a `new_order` notification: the hex encoded order
/// UID as emitted by the `order_insert_notify` database trigger.
fn order_uid_from_notification(payload: &str) -> Option<OrderUid> {
    let bytes = alloy::hex::decode(payload).ok()?;
    Some(OrderUid(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use {super::*, std::sync::Mutex};

    struct Recorder(Arc<Mutex<Vec<OrderUid>>>);

    #[async_trait::async_trait]
    impl Listener for Recorder {
        async fn on_new_order(&self, order: OrderUid) {
            self.0.lock().unwrap().push(order);
        }
    }

    #[tokio::test]
    async fn every_system_sees_every_wellformed_arrival() {
        let first = Arc::new(Mutex::new(Vec::new()));
        let second = Arc::new(Mutex::new(Vec::new()));
        let notifier = Notifier {
            listeners: vec![
                Box::new(Recorder(first.clone())),
                Box::new(Recorder(second.clone())),
            ],
        };

        let order = OrderUid([0x11; 56]);
        notifier.dispatch(&alloy::hex::encode(order.0)).await;
        notifier.dispatch("not an order uid").await;

        assert_eq!(*first.lock().unwrap(), vec![order]);
        assert_eq!(*second.lock().unwrap(), vec![order]);
    }
}
