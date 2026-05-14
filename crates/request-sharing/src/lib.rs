use {
    futures::{
        FutureExt,
        future::{BoxFuture, Shared, WeakShared},
    },
    prometheus::{
        IntCounterVec,
        core::{AtomicU64, GenericGaugeVec},
    },
    std::{
        collections::HashMap,
        future::Future,
        hash::Hash,
        pin::Pin,
        sync::{Arc, Mutex},
        task::{Context, Poll},
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

/// Result of [`RequestSharing::shared_or_else`] indicating whether an
/// already in-flight future was reused or a new one was created.
///
/// Implements [`Future`] so it can be awaited directly.
pub struct SharedResult<Fut: Future> {
    future: Shared<Fut>,
    /// `true` when an existing in-flight request was reused instead of
    /// starting a new one.
    pub is_shared: bool,
}

impl<Fut: Future> Future for SharedResult<Fut>
where
    Fut::Output: Clone,
{
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(cx)
    }
}

type Cache<Request, Response> = Arc<Mutex<HashMap<Request, WeakShared<Response>>>>;

impl<Request: Send + 'static, Fut: Future + Send + 'static> RequestSharing<Request, Fut>
where
    Fut::Output: Send + Sync,
{
    pub fn labelled(request_label: String) -> Self {
        let cache: Cache<Request, Fut> = Default::default();
        Self::spawn_gc(cache.clone(), request_label.clone());
        Self {
            in_flight: cache,
            request_label,
        }
    }

    fn collect_garbage(cache: &Cache<Request, Fut>, label: &str) {
        let mut cache = cache.lock().unwrap();
        cache.retain(|_request, weak| weak.upgrade().is_some());
        Metrics::get()
            .request_sharing_cached_items
            .with_label_values(&[label])
            .set(cache.len() as u64);
    }

    fn spawn_gc(cache: Cache<Request, Fut>, label: String) {
        let weak = Arc::downgrade(&cache);
        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Some(cache) = weak.upgrade() {
                    Self::collect_garbage(&cache, &label);
                } else {
                    return;
                }
            }
        });
    }
}

impl<A, B: Future> Drop for RequestSharing<A, B> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.in_flight) == 1 {
            Metrics::get()
                .request_sharing_cached_items
                .with_label_values(&[&self.request_label])
                .set(0);
        }
    }
}

/// Returns a shallow copy sharing the same in-flight request cache.
impl<Request, Fut: Future> Clone for RequestSharing<Request, Fut> {
    fn clone(&self) -> Self {
        Self {
            in_flight: self.in_flight.clone(),
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
    pub fn shared_or_else<F>(&self, request: Request, future: F) -> SharedResult<Fut>
    where
        F: FnOnce(&Request) -> Fut,
    {
        let mut in_flight = self.in_flight.lock().unwrap();

        let existing = in_flight.get(&request).and_then(WeakShared::upgrade);

        if let Some(existing) = existing {
            Metrics::get()
                .request_sharing_access
                .with_label_values(&[self.request_label.as_str(), "hits"])
                .inc();
            return SharedResult {
                future: existing,
                is_shared: true,
            };
        }

        Metrics::get()
            .request_sharing_access
            .with_label_values(&[self.request_label.as_str(), "misses"])
            .inc();

        let shared = future(&request).shared();
        // unwrap because downgrade only returns None if the Shared has already
        // completed which cannot be the case because we haven't polled it yet.
        in_flight.insert(request, shared.downgrade().unwrap());
        Metrics::get()
            .request_sharing_cached_items
            .with_label_values(&[&self.request_label])
            .set(in_flight.len() as u64);
        SharedResult {
            future: shared,
            is_shared: false,
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Request sharing hits & misses
    #[metric(labels("request_label", "result"))]
    request_sharing_access: IntCounterVec,

    /// Number of all currently cached requests
    #[metric(labels("request_label"))]
    request_sharing_cached_items: GenericGaugeVec<AtomicU64>,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, tokio::runtime::Handle};

    #[tokio::test]
    async fn shares_request() {
        // Manually create [`RequestSharing`] so we can have fine-grain control
        // over the garbage collection.
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "test".to_string();
        let sharing = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };

        let result0 = sharing.shared_or_else(0, |_| futures::future::ready(0).boxed());
        let result1 = sharing.shared_or_else(0, |_| async { panic!() }.boxed());

        assert!(!result0.is_shared);
        assert!(result1.is_shared);

        // Complete first shared — result1 still holds a reference.
        assert_eq!(result0.await, 0);

        // GC does not delete because result1 still references the future.
        RequestSharing::collect_garbage(&sharing.in_flight, &label);
        assert_eq!(sharing.in_flight.lock().unwrap().len(), 1);
        assert!(sharing.in_flight.lock().unwrap().get(&0).is_some());

        // Complete second shared — proves sharing since its factory would panic.
        assert_eq!(result1.await, 0);

        RequestSharing::collect_garbage(&sharing.in_flight, &label);

        // GC deleted all now unused futures.
        assert!(sharing.in_flight.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn in_flight_futures_cache_is_shared_from_origin() {
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "future sharing".to_string();
        let original = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };

        // Create the origin future
        let origin_future = original.shared_or_else(0, |_| futures::future::ready(1u64).boxed());
        assert!(!origin_future.is_shared);

        // The clone should use the original request future, instead of new assignment
        let cloned = original.clone();
        let shared_future = cloned.shared_or_else(0, |_| {
            async { panic!("future cache is not shared") }.boxed()
        });

        // Check origin is reused in shared
        assert!(shared_future.is_shared);

        // Check same value is reached
        assert_eq!(origin_future.await, 1);
        assert_eq!(shared_future.await, 1);
    }

    #[tokio::test]
    async fn in_flight_futures_cache_is_shared_from_clone() {
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "future sharing".to_string();
        let original = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };
        let cloned = original.clone();

        // Create the future on clone
        let cloned_future = cloned.shared_or_else(0, |_| futures::future::ready(1u64).boxed());
        assert!(!cloned_future.is_shared);

        // Origin should use the cloned request future, instead of new assignment
        let origin_future = original.shared_or_else(0, |_| {
            async { panic!("future cache is not shared") }.boxed()
        });
        assert!(origin_future.is_shared);

        // Check same value is yielded
        assert_eq!(cloned_future.await, 1);
        assert_eq!(origin_future.await, 1);
    }

    #[tokio::test]
    async fn gc_cleans_entries_on_clones() {
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "gc shared".to_string();
        let original = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };
        let cloned = original.clone();

        // Create future via the clone and immediately await it.
        let _pending1 = original
            .shared_or_else(0, |_| futures::future::ready(0u64).boxed())
            .await;

        // Create a second future and don't await it, later assertion requires it to be
        // unpolled to survive GC.
        let _pending2 = original.shared_or_else(1, |_| futures::future::ready(0u64).boxed());

        // Run GC
        RequestSharing::collect_garbage(&original.in_flight, &label);

        //Check GC
        assert_eq!(cloned.in_flight.lock().unwrap().len(), 1);
        assert!(!cloned.in_flight.lock().unwrap().contains_key(&0u64));
        assert!(cloned.in_flight.lock().unwrap().contains_key(&1u64));
    }

    #[tokio::test]
    async fn drop_does_not_corrupt_existing_entries() {
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "drop".to_string();
        let original = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };
        let pending = original.shared_or_else(0, |_| futures::future::ready(1u64).boxed());
        {
            let cloned = original.clone();
            let cloned_future = cloned.shared_or_else(0, |_| {
                async { panic!("future cache is not shared") }.boxed()
            });
            // Check cloned_future is shared
            assert!(cloned_future.is_shared);
        } // drop occurs here

        // Check future in cache and that future still yields value
        assert_eq!(original.in_flight.lock().unwrap().len(), 1);
        assert_eq!(pending.await, 1);
    }

    #[tokio::test]
    async fn gc_task_exits_when_all_handles_dropped() {
        let initial_task_count = Handle::current().metrics().num_alive_tasks();

        {
            let _sharing = RequestSharing::<u64, BoxFuture<u64>>::labelled("gc finish".to_string());
            // Yield to let the spawned GC task register.
            tokio::task::yield_now().await;
            assert_eq!(
                Handle::current().metrics().num_alive_tasks(),
                initial_task_count + 1
            );
        } // drop occurs here

        tokio::time::sleep(Duration::from_millis(600)).await;

        let final_task_count = Handle::current().metrics().num_alive_tasks();
        assert_eq!(initial_task_count, final_task_count);
    }

    #[tokio::test]
    async fn gauge_on_clone_drop_not_zeroed() {
        let cache: Cache<u64, BoxFuture<u64>> = Default::default();
        let label = "gauge".to_string();
        let original = RequestSharing {
            in_flight: cache,
            request_label: label.clone(),
        };

        let _pending = original.shared_or_else(0, |_| futures::future::ready(1u64).boxed());
        assert_eq!(Arc::strong_count(&original.in_flight), 1);

        {
            let _cloned = original.clone();
            assert_eq!(Arc::strong_count(&original.in_flight), 2);
        } // drop occurs here

        assert_eq!(Arc::strong_count(&original.in_flight), 1);

        // Since _pending remains, will still have an entry
        assert_eq!(original.in_flight.lock().unwrap().len(), 1);
        drop(original); // will now zero, exact value only testable in integration test
    }
}
