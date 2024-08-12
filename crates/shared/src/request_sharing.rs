use {
    crate::garbage_collector::{GarbageCollecting, GarbageCollector},
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
    },
};

/// Share an expensive to compute response with multiple requests that occur
/// while one of them is already in flight.
pub struct RequestSharing<Request, Fut: Future> {
    in_flight: Arc<Cache<Request, Fut>>,
    request_label: String,
}

/// Request sharing for boxed futures.
pub type BoxRequestSharing<Request, Response> =
    RequestSharing<Request, BoxFuture<'static, Response>>;

/// A boxed shared future.
pub type BoxShared<T> = Shared<BoxFuture<'static, T>>;

/// Cache mapping a request type to a shared future that returns the respective
/// response.
#[derive(Debug)]
struct Cache<Request, Response: Future>(Mutex<HashMap<Request, WeakShared<Response>>>);

impl<Request, Response: Future> Default for Cache<Request, Response> {
    fn default() -> Self {
        Self(Mutex::new(HashMap::default()))
    }
}

impl<Request, Response: Future + Send> GarbageCollecting for Cache<Request, Response>
where
    <Response as Future>::Output: Send + Sync,
    Request: Send,
{
    fn collect_garbage(&self) {
        let mut cache = self.0.lock().unwrap();
        cache.retain(|_request, weak| weak.upgrade().is_some());
    }
}

impl<Request: Send + 'static, Fut: Future + Send + 'static> RequestSharing<Request, Fut>
where
    Fut::Output: Send + Sync,
{
    pub fn labelled(request_label: String, gc: &GarbageCollector) -> Self {
        let cache: Arc<Cache<Request, Fut>> = Arc::new(Default::default());
        gc.trace_memory(&cache);
        Self {
            in_flight: cache,
            request_label,
        }
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
        let mut in_flight = self.in_flight.0.lock().unwrap();

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
        let cache: Arc<Cache<u64, BoxFuture<u64>>> = Default::default();
        let sharing = RequestSharing {
            in_flight: cache.clone(),
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
        cache.collect_garbage();
        assert_eq!(sharing.in_flight.0.lock().unwrap().len(), 1);
        assert!(sharing.in_flight.0.lock().unwrap().get(&0).is_some());

        // complete second shared
        assert_eq!(shared1.now_or_never().unwrap(), 0);

        sharing.in_flight.collect_garbage();

        // GC deleted all now unused futures
        assert!(sharing.in_flight.0.lock().unwrap().is_empty());
    }
}
