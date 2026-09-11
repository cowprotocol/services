use {
    crate::{domain::OrderUid, infra::order_notify::Listener},
    futures::channel::mpsc,
};

/// Forwards each arriving order to the fast-path handler through an
/// unbounded channel. The handler decides whether the order is actually a
/// fast-path order and, if so, initiates the out-of-competition settlement.
pub struct FastPathNotifier(pub mpsc::UnboundedSender<OrderUid>);

#[async_trait::async_trait]
impl Listener for FastPathNotifier {
    async fn on_new_order(&self, order: OrderUid) {
        if let Err(err) = self.0.unbounded_send(order) {
            tracing::error!(?err, "fast-path notification channel is closed");
        }
    }
}
