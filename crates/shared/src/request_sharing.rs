use {
    futures::{
        future::{BoxFuture, Shared, WeakShared},
        FutureExt,
    },
    prometheus::{
        core::{AtomicU64, GenericGauge},
        IntCounterVec,
    },
    std::{
        collections::HashMap,
        future::Future,
        hash::Hash,
        sync::{Arc, Mutex},
        time::Duration,
    },
};

// The design of this module is intentionally simple. Every time a shared future
// is requested we loop through all futures to collect garbage. Because of this
// there is no advantage from using a hash map.
//
// Alternatively we could collect garbage in a background task or return a
// wrapper future that collects garbage on drop. In that case we would use a
// hash map. This alternative approach is more complex and unnecessary because
// we do not expect there to be a large number of futures in flight.

/// Share an expensive to compute response with multiple requests that occur
/// while one of them is already in flight.
pub struct RequestSharing<Request, Fut: Future> {
    in_flight: Arc<Mutex<HashMap<Request, WeakShared<Fut>>>>,
    request_label: String,
}

/// Request sharing for boxed futures.
pub type BoxRequestSharing<Request, Response> =
    RequestSharing<Request, BoxFuture<'static, Response>>;

/// A boxed shared future.
pub type BoxShared<T> = Shared<BoxFuture<'static, T>>;

type Cache<Request, Response> = Arc<Mutex<HashMap<Request, WeakShared<Response>>>>;

impl<Request: Send + 'static, Fut: Future + Send + 'static> RequestSharing<Request, Fut>
where
    Fut::Output: Send + Sync,
{
    pub fn labelled(request_label: String) -> Self {
        let cache: Cache<Request, Fut> = Default::default();
        Self::spawn_gc(cache.clone());
        Self {
            in_flight: cache,
            request_label,
        }
    }

    fn collect_garbage(cache: &Cache<Request, Fut>) {
        let mut cache = cache.lock().unwrap();
        let len_before = cache.len() as u64;
        cache.retain(|_request, weak| weak.upgrade().is_some());
        Metrics::get()
            .request_sharing_cached_items
            .sub(len_before - cache.len() as u64);
    }

    fn spawn_gc(cache: Cache<Request, Fut>) {
        tokio::task::spawn(async move {
            loop {
                Self::collect_garbage(&cache);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
}

impl<A, B: Future> Drop for RequestSharing<A, B> {
    fn drop(&mut self) {
        let cache = self.in_flight.lock().unwrap();
        Metrics::get()
            .request_sharing_cached_items
            .sub(cache.len() as u64);
    }
}

/// Returns a shallow copy (without any pending requests)
impl<Request, Fut: Future> Clone for RequestSharing<Request, Fut> {
    fn clone(&self) -> Self {
        Self {
            in_flight: Default::default(),
            request_label: self.request_label.clone(),
        }
    }
}

impl<Request, Fut> RequestSharing<Request, Fut>
where
    Request: Eq + Hash,
    Fut: Future,
    Fut::Output: Clone,
{
    /// Returns an existing in flight future or creates and uses a new future
    /// from the specified closure.
    pub fn shared_or_else<F>(&self, request: Request, future: F) -> Shared<Fut>
    where
        F: FnOnce(&Request) -> Fut,
    {
        let mut in_flight = self.in_flight.lock().unwrap();

        let existing = in_flight.get(&request).and_then(WeakShared::upgrade);

        if let Some(existing) = existing {
            Metrics::get()
                .request_sharing_access
                .with_label_values(&[&self.request_label, "hits"])
                .inc();
            return existing;
        }

        Metrics::get()
            .request_sharing_access
            .with_label_values(&[&self.request_label, "misses"])
            .inc();

        let shared = future(&request).shared();
        // unwrap because downgrade only returns None if the Shared has already
        // completed which cannot be the case because we haven't polled it yet.
        in_flight.insert(request, shared.downgrade().unwrap());
        Metrics::get().request_sharing_cached_items.inc();
        shared
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Request sharing hits & misses
    #[metric(labels("request_label", "result"))]
    request_sharing_access: IntCounterVec,

    /// Number of all currently cached requests
    request_sharing_cached_items: GenericGauge<AtomicU64>,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shares_request() {
        // Manually create [`RequestSharing`] so we can have fine grained control
        // over the garbage collection.
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let sharing = RequestSharing {
            in_flight: cache,
            request_label: Default::default(),
        };

        let shared0 = sharing.shared_or_else(0, |_| futures::future::ready(0).boxed());
        let shared1 = sharing.shared_or_else(0, |_| async { panic!() }.boxed());

        assert!(shared0.ptr_eq(&shared1));
        assert_eq!(shared0.strong_count().unwrap(), 2);
        assert_eq!(shared1.strong_count().unwrap(), 2);
        assert_eq!(shared0.weak_count().unwrap(), 1);

        // complete first shared
        assert_eq!(shared0.now_or_never().unwrap(), 0);
        assert_eq!(shared1.strong_count().unwrap(), 1);
        assert_eq!(shared1.weak_count().unwrap(), 1);

        // GC does not delete any keys because some tasks still use the future
        RequestSharing::collect_garbage(&sharing.in_flight);
        assert_eq!(sharing.in_flight.lock().unwrap().len(), 1);
        assert!(sharing.in_flight.lock().unwrap().get(&0).is_some());

        // complete second shared
        assert_eq!(shared1.now_or_never().unwrap(), 0);

        RequestSharing::collect_garbage(&sharing.in_flight);

        // GC deleted all now unused futures
        assert!(sharing.in_flight.lock().unwrap().is_empty());
    }
}
