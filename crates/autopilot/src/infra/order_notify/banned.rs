use {
    crate::{domain::OrderUid, infra::order_notify},
    order_validation::banned::Users,
    std::sync::Arc,
};

/// Warms the cache with the owner of every arriving order so the auction cut
/// doesn't pay for the remote lookup on its critical path.
pub struct CachePrewarmer(pub Arc<Users>);

#[async_trait::async_trait]
impl order_notify::Listener for CachePrewarmer {
    async fn on_new_order(&self, order: OrderUid) {
        let owner = order.owner();
        self.0.banned([owner]).await;
    }
}
