//! A buffered implementation that automatically groups native prices API
//! requests into batches.

use {
    crate::price_estimation::{
        native::{NativePriceEstimateResult, NativePriceEstimating},
        Metrics,
        PriceEstimationError,
    },
    anyhow::anyhow,
    futures::{
        channel::mpsc,
        future::FutureExt as _,
        stream::{self, FusedStream, Stream, StreamExt as _},
    },
    primitive_types::H160,
    std::{
        collections::{HashMap, HashSet},
        future::Future,
        num::NonZeroUsize,
        sync::Arc,
        time::Duration,
    },
    tokio::{sync::broadcast, task::JoinHandle},
};

/// Buffered configuration.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Configuration {
    /// The maximum amount of concurrent batches to request.
    ///
    /// Specifying `None` means no limit on concurrency.
    pub max_concurrent_requests: Option<NonZeroUsize>,
    /// An additional minimum delay to wait for collecting requests.
    ///
    /// The delay to start counting after receiving the first request.
    pub debouncing_time: Duration,
    /// The timeout to wait for the result to be ready
    pub result_ready_timeout: Duration,
    /// Maximum capacity of the broadcast channel to store the native prices
    /// results
    pub broadcast_channel_capacity: usize,
}

/// Trait for fetching a batch of native price estimates.
#[allow(dead_code)]
#[cfg_attr(test, mockall::automock)]
pub trait NativePriceBatchFetching: Sync + Send + NativePriceEstimating {
    /// Fetches a batch of native price estimates.
    ///
    /// It returns a HashMap which maps the token with its native price
    /// estimator result
    fn fetch_native_prices(
        &self,
        tokens: HashSet<H160>,
    ) -> futures::future::BoxFuture<
        '_,
        Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError>,
    >;

    /// Returns the number of prices that can be fetched in a single batch.
    fn max_batch_size(&self) -> usize;
}

/// Buffered implementation that implements automatic batching of
/// native prices requests.
#[allow(dead_code)]
#[derive(Clone)]
pub struct BufferedRequest<Inner> {
    config: Configuration,
    inner: Arc<Inner>,
    requests: mpsc::UnboundedSender<H160>,
    results: broadcast::Sender<NativePriceResult>,
}

/// Object to map the token with its native price estimator result
#[allow(dead_code)]
#[derive(Clone)]
struct NativePriceResult {
    token: H160,
    result: Result<f64, PriceEstimationError>,
}

impl<Inner> NativePriceEstimating for BufferedRequest<Inner>
where
    Inner: NativePriceBatchFetching + NativePriceEstimating + 'static,
{
    /// Request to get estimate prices in a batch
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            Metrics::inc_estimator("buffered");
            // We must subscribe before we send the request, so we get the `rx` pointing to
            // the current memory point, this way we avoid losing the result for
            // the corner case in which the request is sent between the `unbounded_send()`
            // and the `subscribe()`
            let mut rx = self.results.subscribe();

            // Sends the token for requesting price
            self.requests.unbounded_send(token).map_err(|e| {
                PriceEstimationError::ProtocolInternal(anyhow!(
                    "failed to append a new token to the queue: {e:?}"
                ))
            })?;

            tokio::time::timeout(self.config.result_ready_timeout, async {
                loop {
                    match rx.recv().await {
                        Ok(value) => {
                            if value.token == token {
                                return value.result;
                            }
                        }
                        // Receiver lagged behind the result stream but the
                        // necessary response might still be in the stream.
                        Err(_) => continue,
                    }
                }
            })
            .await
            .map_err(|_| {
                PriceEstimationError::ProtocolInternal(anyhow!(
                    "blocking buffered estimate prices timeout elapsed"
                ))
            })?
        }
        .boxed()
    }
}

#[allow(dead_code)]
impl<Inner> BufferedRequest<Inner>
where
    Inner: NativePriceBatchFetching + Send + Sync + NativePriceEstimating + 'static,
{
    /// Creates a new buffered transport with the specified configuration.
    pub fn with_config(inner: Inner, config: Configuration) -> Self {
        let inner = Arc::new(inner);
        let (requests_sender, requests_receiver) = mpsc::unbounded();

        let (results_sender, _) = broadcast::channel(config.broadcast_channel_capacity);

        Self::background_worker(
            inner.clone(),
            config.clone(),
            requests_receiver,
            results_sender.clone(),
        );

        Self {
            inner,
            requests: requests_sender,
            results: results_sender,
            config,
        }
    }

    /// Start a background worker for handling batched requests.
    fn background_worker(
        inner: Arc<Inner>,
        config: Configuration,
        requests: mpsc::UnboundedReceiver<H160>,
        results_sender: broadcast::Sender<NativePriceResult>,
    ) -> JoinHandle<()> {
        tokio::task::spawn(batched_for_each(
            config,
            requests,
            inner.max_batch_size(),
            move |batch| {
                let inner = inner.clone();
                let results_sender = results_sender.clone();
                async move {
                    if batch.is_empty() {
                        return;
                    }
                    let batch_map = batch.iter().cloned().collect::<HashSet<_>>();
                    let results: Vec<_> = match inner.fetch_native_prices(batch_map).await {
                        Ok(results) => results
                            .into_iter()
                            .map(|(token, price)| NativePriceResult {
                                token,
                                result: price,
                            })
                            .collect(),
                        Err(err) => {
                            tracing::error!(?err, "failed to send native price batch request");
                            batch
                                .into_iter()
                                .map(|token| NativePriceResult {
                                    token,
                                    result: Err(err.clone()),
                                })
                                .collect()
                        }
                    };
                    for result in results {
                        let _ = results_sender.send(result);
                    }
                }
            },
        ))
    }
}

/// Batches a stream into chunks.
///
/// This is very similar to `futures::stream::StreamExt::ready_chunks` with the
/// difference that it allows configuring a minimum delay for a batch, so
/// waiting for a small amount of time to allow the stream to produce additional
/// items, thus decreasing the chance of batches of size 1.
fn batched_for_each<T, St, F, Fut>(
    config: Configuration,
    items: St,
    max_batch_size: usize,
    work: F,
) -> impl Future<Output = ()>
where
    St: Stream<Item = T> + FusedStream + Unpin,
    F: Fn(Vec<T>) -> Fut,
    Fut: Future<Output = ()>,
{
    let concurrency_limit = config.max_concurrent_requests.map(NonZeroUsize::get);

    let batches = stream::unfold(items, move |mut items| async move {
        let mut chunk = vec![items.next().await?];

        let delay = tokio::time::sleep(config.debouncing_time).fuse();
        futures::pin_mut!(delay);

        // Append new elements to the bulk until reaching either of the scenarios:
        // - reach maximum number of elements per batch (`max_batch_len)
        // - we reach the `debouncing_time`
        while chunk.len() < max_batch_size {
            futures::select_biased! {
                item = items.next() => match item {
                    Some(item) => chunk.push(item),
                    None => break,
                },
                _ = delay => break,
            }
        }

        Some((chunk, items))
    });

    batches.for_each_concurrent(concurrency_limit, work)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::native::MockNativePriceEstimating,
        futures::future::try_join_all,
        num::ToPrimitive,
        tokio::time::sleep,
    };

    impl NativePriceEstimating for MockNativePriceBatchFetching {
        fn estimate_native_price(
            &self,
            token: H160,
        ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
            async move {
                let prices = self.fetch_native_prices(HashSet::from([token])).await?;
                prices
                    .get(&token)
                    .cloned()
                    .ok_or(PriceEstimationError::NoLiquidity)?
            }
            .boxed()
        }
    }

    fn token(u: u64) -> H160 {
        H160::from_low_u64_be(u)
    }

    #[tokio::test]
    async fn single_batch_request_successful_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            // Because it gets the value from the batch estimator, it does not need to do this call at all
            .never();

        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                let input_cloned = input.clone();
                async move {
                    Ok(input_cloned
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
                }.boxed()
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = BufferedRequest::with_config(native_price_batch_fetcher, config);
        let result = buffered.estimate_native_price(token(0)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_successful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                let input_cloned = input.clone();
                async move { Ok(input_cloned
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>()) }.boxed()
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = BufferedRequest::with_config(native_price_batch_fetcher, config);

        let result = buffered.estimate_native_price(token(0)).await;

        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_unsuccessful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one
            .times(1)
            .returning(|_| {
                async { Err(PriceEstimationError::NoLiquidity) }.boxed()
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = BufferedRequest::with_config(native_price_batch_fetcher, config);

        let result = buffered.estimate_native_price(token(0)).await;

        assert_eq!(result, Err(PriceEstimationError::NoLiquidity));
    }

    // Function to check batching of many tokens
    async fn check_batching_many(
        buffered: Arc<BufferedRequest<MockNativePriceBatchFetching>>,
        tokens_requested: usize,
    ) {
        let mut futures = Vec::with_capacity(tokens_requested);
        for i in 0..tokens_requested {
            let buffered = buffered.clone();
            futures.push(tokio::spawn(async move {
                buffered
                    .estimate_native_price(token(i.try_into().unwrap()))
                    .await
            }));
        }

        let mut results = try_join_all(futures).await.expect(
            "valid
    futures",
        );

        while let Some(result) = results.pop() {
            let result = result.unwrap();
            assert_eq!(result.to_i64().unwrap(), 1);
        }
    }

    #[tokio::test]
    async fn batching_many_in_one_batch_successful_estimates() {
        let tokens_requested = 20;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(move || tokens_requested);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly one time because the max batch is 20, so all petitions fit into one batch request
            .times(1)
            .returning(|input| {
                let input_cloned = input.clone();
                async move { Ok(input_cloned
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>()) }.boxed()
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = Arc::new(BufferedRequest::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_many_in_one_batch_with_mixed_results_estimates() {
        let tokens_requested = 2;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly one time because the max batch is 20, so all petitions fit into one batch request
            .times(1)
            .returning(|input| {
                let input_cloned = input.clone();
                async move { Ok(input_cloned
                    .iter()
                    .enumerate()
                    .map(|(i, token)|
                        if i % 2 == 0 {
                            (*token, Ok::<_, PriceEstimationError>(1.0))
                        } else {
                            (*token, Err(PriceEstimationError::NoLiquidity))
                        }
                    ).collect::<HashMap<_, _>>()) }.boxed()
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = Arc::new(BufferedRequest::with_config(
            native_price_batch_fetcher,
            config,
        ));

        let mut futures = Vec::with_capacity(tokens_requested);
        for i in 0..tokens_requested {
            let buffered = buffered.clone();
            futures.push(tokio::spawn(async move {
                buffered
                    .estimate_native_price(token(i.try_into().unwrap()))
                    .await
            }));
        }

        let results = try_join_all(futures).await.expect(
            "valid
    futures",
        );

        // We got two results, one must be correct and the other with an error
        assert_eq!(results.len(), 2);
        assert!(results.contains(&Ok(1.0)));
        assert!(results.contains(&Err(PriceEstimationError::NoLiquidity)));
    }

    #[tokio::test]
    async fn batching_many_in_two_batch_successful_estimates() {
        let tokens_requested = 21;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly two times because the max batch is 20, so all petitions fit into one batch request
            .times(2)
            .returning(|input| {
                let input_cloned = input.clone();
                async move { Ok(input_cloned
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>()) }.boxed()
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = Arc::new(BufferedRequest::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_no_calls() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We are testing the native prices are never called
            .never();
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            debouncing_time: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let _buffered = Arc::new(BufferedRequest::with_config(
            native_price_batch_fetcher,
            config,
        ));

        sleep(Duration::from_millis(250)).await;
    }

    #[tokio::test]
    async fn batching_many_in_multiple_times_successful_estimates() {
        let tokens_requested = 20;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetching::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly two times because there are two batches petitions separated by 250 ms
            .times(2)
            .returning(|input| {
                let input_cloned = input.clone();
                async move { Ok(input_cloned
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>()) }.boxed()
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            debouncing_time: Duration::from_millis(10),
            result_ready_timeout: Duration::from_millis(500),
            broadcast_channel_capacity: 50,
        };

        let buffered = Arc::new(BufferedRequest::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered.clone(), tokens_requested).await;

        sleep(Duration::from_millis(20)).await;

        check_batching_many(buffered, tokens_requested).await;
    }
}
