pub use order_validation::banned::*;
use {
    crate::domain::order_notify,
    futures::StreamExt,
    std::sync::Arc,
    tokio_stream::wrappers::errors::BroadcastStreamRecvError,
};

/// Spawns a task that warms the cache with the owner of every arriving order
/// so the auction cut doesn't pay for the remote lookup on its critical path.
///
/// Best effort: the receiver of an order is not part of its UID, so it remains
/// a cut time lookup.
pub fn spawn_cache_prewarming(arrivals: &order_notify::Notifier, users: Arc<Users>) {
    let mut arrivals = arrivals.subscribe();
    tokio::spawn(async move {
        while let Some(arrival) = arrivals.next().await {
            let owner = match arrival {
                Ok(order) => order.owner(),
                Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                    tracing::debug!(skipped, "lagged behind new orders, skipping prewarming");
                    continue;
                }
            };
            // Owners we already know about make up the bulk of the arrivals.
            if users.cached(&owner).is_none() {
                users.banned([owner]).await;
            }
        }
        tracing::error!("banned users cache prewarming task terminated unexpectedly");
    });
}
