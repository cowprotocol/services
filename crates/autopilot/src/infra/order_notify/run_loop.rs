use {
    crate::{domain::OrderUid, infra::order_notify::Listener},
    futures::channel::mpsc,
};

/// "Wakes" up (i.e. notifies) the run-loop to start when a new block or order
/// appears.
pub struct RunLoopWaker(pub mpsc::UnboundedSender<OrderUid>);

#[async_trait::async_trait]
impl Listener for RunLoopWaker {
    async fn on_new_order(&self, order: OrderUid) {
        self.0.unbounded_send(order).unwrap()
    }
}
