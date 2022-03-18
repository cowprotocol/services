use crate::price_estimation::{
    old_estimator_to_stream, vec_estimates, PriceEstimateResult, PriceEstimating, Query,
};
use futures::{future::WeakShared, FutureExt};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

type SharedEstimationRequest =
    WeakShared<Pin<Box<dyn Future<Output = PriceEstimateResult> + Send>>>;

struct Inner {
    estimator: Box<dyn PriceEstimating>,
    in_flight_requests: Mutex<HashMap<Query, SharedEstimationRequest>>,
}

impl Inner {
    fn collect_garbage_with_lock(
        active_requests: &mut MutexGuard<HashMap<Query, SharedEstimationRequest>>,
    ) {
        // TODO: Refactor this to use `HashMap::drain_filter` when it's stable:
        // https://github.com/rust-lang/rust/issues/59618
        let completed_and_ignored_requests: Vec<_> = active_requests
            .iter()
            .filter_map(|(query, handle)| match handle.upgrade() {
                // future terminated
                Some(fut) if fut.peek().is_some() => Some(*query),
                // no strong references left
                None => Some(*query),
                // something still depends on the future making progress
                Some(_) => None,
            })
            .collect();
        for query in &completed_and_ignored_requests {
            active_requests.remove(query);
        }
    }

    #[cfg(test)]
    fn collect_garbage(&self) {
        Self::collect_garbage_with_lock(&mut self.in_flight_requests.lock().unwrap());
    }
}

/// A price estimator which doesn't issue another estimation request while an identical one is
/// already in-flight.
#[derive(Clone)]
pub struct BufferingPriceEstimator {
    inner: Arc<Inner>,
}

impl BufferingPriceEstimator {
    pub fn new(estimator: Box<dyn PriceEstimating>) -> Self {
        Self {
            inner: Arc::new(Inner {
                estimator,
                in_flight_requests: Mutex::new(Default::default()),
            }),
        }
    }

    async fn estimates_(&self, queries: &[Query]) -> impl Iterator<Item = PriceEstimateResult> {
        let (active_requests, new_requests) = {
            let mut in_flight_requests = self.inner.in_flight_requests.lock().unwrap();

            Inner::collect_garbage_with_lock(&mut in_flight_requests);

            // For each `Query` either get an in-flight request or keep the `Query` to forward it to the
            // inner price estimator.
            let (active_requests, remaining_queries): (Vec<_>, Vec<_>) = queries
                .iter()
                .map(
                    |query| match in_flight_requests.get(query).map(WeakShared::upgrade) {
                        // NOTE: Technically it's possible under very specific circumstances
                        // that the `active_request` is sitting in the cache for a long time
                        // without making progress. If somebody else picks it up and polls
                        // it to completion a timeout error will most likely be the result.
                        // See https://github.com/gnosis/gp-v2-services/pull/1677#discussion_r813673692
                        // for more details.
                        Some(Some(active_request)) => (Some(active_request), None),
                        _ => (None, Some(*query)),
                    },
                )
                .unzip();

            // Create future which estimates all `remaining_queries` in a single batch.
            let fetch_remaining_estimates = {
                let remaining_queries: Vec<_> =
                    remaining_queries.iter().flatten().cloned().collect();
                let inner = self.inner.clone();
                async move { vec_estimates(inner.estimator.as_ref(), &remaining_queries).await }
                    .boxed()
                    .shared()
            };

            // Create a `SharedEstimationRequest` for each individual `Query` of the batch. This
            // makes it possible for a `batch_2` to await the queries which it is interested in of the
            // in-flight `batch_1`. Even if the estimator which requested `batch_1` stops polling it, the
            // estimator of `batch_2` can still poll `batch_1` to completion by polling the
            // `SharedEstimationRequest` it is actually interested in.
            let new_requests: Vec<_> = remaining_queries
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(index, query)| {
                    let fetch_remaining_estimates = fetch_remaining_estimates.clone();
                    (
                        query,
                        async move { fetch_remaining_estimates.await[index].clone() }
                            .boxed()
                            .shared(),
                    )
                })
                .collect();
            in_flight_requests.extend(
                new_requests
                    .iter()
                    .map(|(query, fut)| (*query, fut.downgrade().expect("future completed"))),
            );

            (active_requests, new_requests)
        };

        // Await all the estimates we need (in-flight and the new ones) in parallel.
        let results = futures::join!(
            futures::future::join_all(active_requests.iter().flatten().cloned()),
            futures::future::join_all(new_requests.into_iter().map(|(_, query)| query)),
        );
        let (mut in_flight_results, mut new_results) =
            (results.0.into_iter(), results.1.into_iter());

        // Return the results of new and in-flight requests merged into one.
        active_requests
            .into_iter()
            .map(move |request| match request {
                Some(_) => in_flight_results.next().unwrap(),
                None => new_results.next().unwrap(),
            })
    }
}

impl PriceEstimating for BufferingPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        old_estimator_to_stream(self.estimates_(queries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{old_estimator_to_stream, Estimate, MockPriceEstimating};
    use futures::poll;
    use maplit::hashset;
    use primitive_types::H160;
    use std::collections::HashSet;
    use std::time::Duration;
    use tokio::time::sleep;

    fn in_flight_requests(buffered: &BufferingPriceEstimator) -> HashSet<Query> {
        HashSet::from_iter(
            buffered
                .inner
                .in_flight_requests
                .lock()
                .unwrap()
                .keys()
                .cloned(),
        )
    }

    #[tokio::test]
    async fn request_can_be_completed_by_request_depending_on_it() {
        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }
        let query = |address| Query {
            sell_token: H160::from_low_u64_be(address),
            ..Default::default()
        };

        let first_batch = [query(1), query(2)];
        let second_batch = [query(2), query(3)];

        let mut estimator = Box::new(MockPriceEstimating::new());
        estimator
            .expect_estimates()
            .times(1)
            .returning(move |queries| {
                assert_eq!(queries, first_batch);
                old_estimator_to_stream(async {
                    sleep(Duration::from_millis(10)).await;
                    [Ok(estimate(1)), Ok(estimate(2))]
                })
            });

        estimator
            .expect_estimates()
            .times(1)
            .returning(move |queries| {
                // only the missing query actually needs to be estimated
                assert_eq!(queries, &vec![query(3)]);
                old_estimator_to_stream(async {
                    sleep(Duration::from_millis(10)).await;
                    [Ok(estimate(3))]
                })
            });

        let buffered = BufferingPriceEstimator::new(estimator);
        let first_batch_request = vec_estimates(&buffered, &first_batch).shared();
        let second_batch_request = vec_estimates(&buffered, &second_batch).shared();

        assert!(buffered.inner.in_flight_requests.lock().unwrap().is_empty());

        // Poll first batch to store futures for its inidividual queries.
        let _ = poll!(first_batch_request.clone());
        assert_eq!(
            in_flight_requests(&buffered),
            hashset! { query(1), query(2) }
        );

        // Poll second batch to store futures for its NEW inidividual queries.
        let _ = poll!(second_batch_request.clone());
        assert_eq!(
            in_flight_requests(&buffered),
            hashset! { query(1), query(2), query(3) }
        );

        drop(first_batch_request);
        // Drop all futures which nobody depends on anymore.
        buffered.inner.collect_garbage();
        assert_eq!(
            in_flight_requests(&buffered),
            hashset! { query(2), query(3) }
        );

        // Poll second future to completion.
        let second_batch_result = second_batch_request.await;
        assert_eq!(second_batch_result.len(), 2);
        // Although the initiator of the request for `query(2)` dropped its future, other futures
        // depending on the result can still drive the original future to completion.
        assert_eq!(second_batch_result[0].as_ref().unwrap(), &estimate(2));
        assert_eq!(second_batch_result[1].as_ref().unwrap(), &estimate(3));

        buffered.inner.collect_garbage();
        assert!(buffered.inner.in_flight_requests.lock().unwrap().is_empty());
    }
}
