//! A buffered implementation that automatically groups native prices API
//! requests into batches.

use {
    crate::price_estimation::{native::NativePriceEstimateResult, PriceEstimationError},
    anyhow::anyhow,
    async_trait::async_trait,
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
    tokio::{sync::broadcast, task::JoinHandle, time::error::Elapsed},
};

/// Buffered configuration.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Configuration {
    /// The maximum amount of concurrent batches to request.
    ///
    /// Specifying `None` means no limit on concurrency.
    pub max_concurrent_requests: Option<NonZeroUsize>,
    /// The maximum batch size.
    pub max_batch_len: usize,
    /// An additional minimum delay to wait for collecting requests.
    ///
    /// The delay starts counting after receiving the first request.
    pub batch_delay: Duration,
    /// The timeout to wait for the result to be ready
    pub result_ready_timeout: Duration,
}

/// Trait for fetching a batch of native price estimates.
#[allow(dead_code)]
#[mockall::automock]
#[async_trait]
pub trait NativePriceBatchFetcher: Sync + Send {
    /// Fetches a batch of native price estimates.
    ///
    /// It returns a HashMap which maps the token with its native price
    /// estimator result
    async fn fetch_native_prices(
        &self,
        tokens: &HashSet<H160>,
    ) -> Result<HashMap<H160, Result<f64, PriceEstimationError>>, PriceEstimationError>;
}

/// Buffered implementation that implements automatic batching of
/// native prices requests.
#[allow(dead_code)]
#[derive(Clone)]
pub struct BufferedTransport<Inner> {
    config: Configuration,
    inner: Arc<Inner>,
    calls: mpsc::UnboundedSender<H160>,
    broadcast_sender: broadcast::Sender<NativePriceResult>,
}

/// Object to map the token with its native price estimator result
#[allow(dead_code)]
#[derive(Clone)]
struct NativePriceResult {
    token: H160,
    result: Result<f64, PriceEstimationError>,
}

#[allow(dead_code)]
impl<Inner> BufferedTransport<Inner>
where
    Inner: NativePriceBatchFetcher + Send + Sync + 'static,
{
    /// Maximum capacity of the broadcast channel, the messages are discarded as
    /// soon as they are sent, so this limit should be enough to hold the
    /// flow
    const BROADCAST_CHANNEL_CAPACITY: usize = 50;

    /// Creates a new buffered transport with the specified configuration.
    pub fn with_config(inner: Inner, config: Configuration) -> Self {
        let inner = Arc::new(inner);
        let (calls, receiver) = mpsc::unbounded();

        let (broadcast_sender, _) = broadcast::channel(Self::BROADCAST_CHANNEL_CAPACITY);

        Self::background_worker(
            inner.clone(),
            config.clone(),
            receiver,
            broadcast_sender.clone(),
        );

        Self {
            inner,
            calls,
            broadcast_sender,
            config,
        }
    }

    /// Start a background worker for handling batched requests.
    fn background_worker(
        inner: Arc<Inner>,
        config: Configuration,
        calls: mpsc::UnboundedReceiver<H160>,
        broadcast_sender: broadcast::Sender<NativePriceResult>,
    ) -> JoinHandle<()> {
        tokio::task::spawn(batched_for_each(config, calls, move |batch| {
            let inner = inner.clone();
            let broadcast_sender = broadcast_sender.clone();
            async move {
                let batch = batch.into_iter().collect::<HashSet<_>>();
                if !batch.is_empty() {
                    let results: Vec<_> = match inner.fetch_native_prices(&batch).await {
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
                        let _ = broadcast_sender.send(result);
                    }
                }
            }
        }))
    }

    /// Request to get estimate prices in a batch
    pub async fn request_buffered_estimate_prices(
        &self,
        token: &H160,
    ) -> NativePriceEstimateResult {
        // Sends the token for requesting price
        self.calls.unbounded_send(*token).map_err(|e| {
            PriceEstimationError::ProtocolInternal(anyhow!(
                "failed to append a new token to the queue: {e:?}"
            ))
        })?;

        let mut rx = self.broadcast_sender.subscribe();

        tokio::time::timeout(self.config.result_ready_timeout, async {
            loop {
                if let Ok(Some(result)) =
                    Self::receive_with_timeout(&mut rx, token, self.config.batch_delay).await
                {
                    return result.result;
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

    // Function to receive with a timeout
    async fn receive_with_timeout(
        rx: &mut broadcast::Receiver<NativePriceResult>,
        token: &H160,
        timeout_duration: Duration,
    ) -> Result<Option<NativePriceResult>, Elapsed> {
        tokio::time::timeout(timeout_duration, async {
            match rx.recv().await {
                Ok(value) => (value.token == *token).then_some(value),
                Err(_) => None,
            }
        })
        .await
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

        let delay = tokio::time::sleep(config.batch_delay).fuse();
        futures::pin_mut!(delay);

        while chunk.len() < config.max_batch_len {
            futures::select_biased! {
                item = items.next() => match item {
                    Some(item) => {
                        chunk.push(item);
                    }
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

        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);
        let result = buffered.request_buffered_estimate_prices(&token(0)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_successful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);

        let result = buffered.request_buffered_estimate_prices(&token(0)).await;

        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_unsuccessful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one
            .times(1)
            .returning(|_| {
                Err(PriceEstimationError::NoLiquidity)
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);

        let result = buffered.request_buffered_estimate_prices(&token(0)).await;

        assert_eq!(result, Err(PriceEstimationError::NoLiquidity));
    }

    // Function to check batching of many tokens
    async fn check_batching_many(
        buffered: Arc<BufferedTransport<MockNativePriceBatchFetcher>>,
        tokens_requested: usize,
    ) {
        let mut futures = Vec::with_capacity(tokens_requested);
        for i in 0..tokens_requested {
            let buffered = buffered.clone();
            futures.push(tokio::spawn(async move {
                buffered
                    .request_buffered_estimate_prices(&token(i.try_into().unwrap()))
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
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly one time because the max batch is 20, so all petitions fit into one batch request
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_many_in_one_batch_with_mixed_results_estimates() {
        let tokens_requested = 2;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly one time because the max batch is 20, so all petitions fit into one batch request
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .enumerate()
                    .map(|(i, token)|
                        if i % 2 == 0 {
                            (*token, Ok::<_, PriceEstimationError>(1.0))
                        } else {
                            (*token, Err(PriceEstimationError::NoLiquidity))
                        }
                    ).collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        let mut futures = Vec::with_capacity(tokens_requested);
        for i in 0..tokens_requested {
            let buffered = buffered.clone();
            futures.push(tokio::spawn(async move {
                buffered
                    .request_buffered_estimate_prices(&token(i.try_into().unwrap()))
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
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly two times because the max batch is 20, so all petitions fit into one batch request
            .times(2)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_no_calls() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We are testing the native prices are never called
            .never();
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let _buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        sleep(Duration::from_millis(250)).await;
    }

    #[tokio::test]
    async fn batching_many_in_multiple_times_successful_estimates() {
        let tokens_requested = 20;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly two times because there are two batches petitions separated by 250 ms
            .times(2)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, Ok::<_, PriceEstimationError>(1.0)))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
            result_ready_timeout: Duration::from_millis(500),
        };

        let buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered.clone(), tokens_requested).await;

        sleep(Duration::from_millis(200)).await;

        check_batching_many(buffered, tokens_requested).await;
    }
}
