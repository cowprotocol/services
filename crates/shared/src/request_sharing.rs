use {
    futures::{
        future::{BoxFuture, Shared, WeakShared},
        FutureExt,
    },
    std::{future::Future, sync::Mutex},
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
    in_flight: Mutex<Vec<(Request, WeakShared<Fut>)>>,
}

/// Request sharing for boxed futures.
pub type BoxRequestSharing<Request, Response> =
    RequestSharing<Request, BoxFuture<'static, Response>>;

/// A boxed shared future.
pub type BoxShared<T> = Shared<BoxFuture<'static, T>>;

impl<Request, Fut: Future> Default for RequestSharing<Request, Fut> {
    fn default() -> Self {
        Self {
            in_flight: Default::default(),
        }
    }
}

impl<Request, Fut> RequestSharing<Request, Fut>
where
    Request: Eq,
    Fut: Future,
    Fut::Output: Clone,
{
    // Intentionally returns Shared<Fut> instead of an opaque `impl Future` (or
    // being an async fn) because this has some useful properties to the caller
    // like being unpin and fused.

    /// Returns an existing in flight future for this request or uses the passed
    /// in future as a new in flight future.
    ///
    /// Note that futures do nothing util polled so merely creating the response
    /// future is not expensive.
    pub fn shared(&self, request: Request, future: Fut) -> Shared<Fut> {
        self.shared_or_else(request, move |_| future)
    }

    /// Returns an existing in flight future or creates and uses a new future
    /// from the specified closure.
    ///
    /// This is similar to [`RequestSharing::shared`] but lazily creates the
    /// future. This can be helpful when creating futures is non trivial
    /// (such as cloning a large vector).
    pub fn shared_or_else<F>(&self, request: Request, future: F) -> Shared<Fut>
    where
        F: FnOnce(&Request) -> Fut,
    {
        let mut in_flight = self.in_flight.lock().unwrap();

        // collect garbage and find copy of existing request
        let mut existing = None;
        in_flight.retain(|(request_, weak)| match weak.upgrade() {
            // NOTE: Technically it's possible under very specific circumstances that the
            // `active_request` is sitting in the cache for a long time without making progress.
            // If somebody else picks it up and polls it to completion a timeout error will most
            // likely be the result. See https://github.com/gnosis/gp-v2-services/pull/1677#discussion_r813673692
            // for more details.
            Some(shared) if shared.peek().is_none() => {
                if *request_ == request {
                    debug_assert!(existing.is_none());
                    existing = Some(shared);
                }
                true
            }
            _ => false,
        });

        if let Some(existing) = existing {
            return existing;
        }

        let shared = future(&request).shared();
        // unwrap because downgrade only returns None if the Shared has already
        // completed which cannot be the case because we haven't polled it yet.
        in_flight.push((request, shared.downgrade().unwrap()));
        shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_request() {
        let sharing = RequestSharing::default();
        let shared0 = sharing.shared(0, futures::future::ready(0).boxed());
        let shared1 = sharing.shared(0, async { panic!() }.boxed());
        // Would use Arc::ptr_eq but Shared doesn't implement it.
        assert_eq!(shared0.strong_count().unwrap(), 2);
        assert_eq!(shared1.strong_count().unwrap(), 2);
        assert_eq!(shared0.weak_count().unwrap(), 1);
        // complete first shared
        assert_eq!(shared0.now_or_never().unwrap(), 0);
        assert_eq!(shared1.strong_count().unwrap(), 1);
        assert_eq!(shared1.weak_count().unwrap(), 1);
        // garbage collect completed future, same request gets assigned new future
        let shared3 = sharing.shared(0, futures::future::ready(1).boxed());
        assert_eq!(shared3.now_or_never().unwrap(), 1);
        // previous future still works
        assert_eq!(shared1.strong_count().unwrap(), 1);
        assert_eq!(shared1.weak_count().unwrap(), 0);
        // complete second shared
        assert_eq!(shared1.now_or_never().unwrap(), 0);
    }
}
