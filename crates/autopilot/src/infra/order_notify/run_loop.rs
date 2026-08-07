use {
    crate::{domain::OrderUid, infra::order_notify::Listener},
    std::sync::Arc,
};

/// Wakes the run loop so the arriving order makes it into the next auction as
/// soon as possible.
pub struct RunLoopWaker(pub Arc<tokio::sync::Notify>);

#[async_trait::async_trait]
impl Listener for RunLoopWaker {
    async fn on_new_order(&self, _: OrderUid) {
        self.0.notify_one();
    }
}
