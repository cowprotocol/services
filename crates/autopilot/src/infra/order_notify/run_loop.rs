use {
    crate::{domain::OrderUid, infra::order_notify::Listener},
    std::sync::Arc,
};

/// "Wakes" up (i.e. notifies) the run-loop to start when a new block or order
/// appears.
pub struct RunLoopWaker(pub Arc<tokio::sync::Notify>);

#[async_trait::async_trait]
impl Listener for RunLoopWaker {
    async fn on_new_order(&self, _: OrderUid) {
        self.0.notify_one();
    }
}
