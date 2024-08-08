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
    tokio::{
        sync::broadcast,
        task::JoinHandle,
        time::{error::Elapsed, sleep},
    },
};

/// Buffered configuration.
#[derive(Clone)]
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
}

/// Trait for fetching a batch of native price estimates.
#[mockall::automock]
#[async_trait]
pub trait NativePriceBatchFetcher: Sync + Send {
    /// Fetches a batch of native price estimates.
    ///
    /// It returns a HashMap which maps the token with its price
    async fn fetch_native_prices(
        &self,
        tokens: &HashSet<H160>,
    ) -> Result<HashMap<H160, f64>, PriceEstimationError>;
}

/// Buffered implementation that implements automatic batching of
/// native prices requests.
#[derive(Clone)]
pub struct BufferedTransport<Inner> {
    config: Configuration,
    #[allow(dead_code)]
    inner: Arc<Inner>,
    calls: mpsc::UnboundedSender<H160>,
    broadcast_sender: broadcast::Sender<NativePriceResult>,
}

type NativePriceResult = (H160, Result<f64, PriceEstimationError>);

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
                if batch.len() != 0 {
                    let results = match inner.fetch_native_prices(&batch).await {
                        Ok(results) => results
                            .into_iter()
                            .map(|(token, price)| (token, Ok::<_, PriceEstimationError>(price)))
                            .collect::<HashMap<_, _>>(),
                        Err(err) => {
                            tracing::error!(?err, "failed to send native price batch request");
                            batch
                                .into_iter()
                                .map(|token| (token, Err(err.clone())))
                                .collect::<HashMap<_, _>>()
                        }
                    };
                    for result in results {
                        let _ = broadcast_sender.send(result);
                    }
                }
            }
        }))
    }

    /// Blocking operation to get estimate prices in a batch
    pub async fn blocking_buffered_estimate_prices(
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

        tokio::time::timeout(self.config.batch_delay.saturating_mul(3), async {
            loop {
                if let Ok(Some(result)) =
                    Self::receive_with_timeout(&mut rx, token, self.config.batch_delay).await
                {
                    return result.1;
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
                Ok(value) => {
                    if value.0 == *token {
                        Some(value)
                    } else {
                        None
                    }
                }
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
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);
        let result = buffered.blocking_buffered_estimate_prices(&token(0)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_successful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the econd call it fetches the cached one
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);

        let result = buffered.blocking_buffered_estimate_prices(&token(0)).await;

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
        };

        let buffered = BufferedTransport::with_config(native_price_batch_fetcher, config);

        let result = buffered.blocking_buffered_estimate_prices(&token(0)).await;

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
                    .blocking_buffered_estimate_prices(&token(i.try_into().unwrap()))
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
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
        };

        let buffered = Arc::new(BufferedTransport::with_config(
            native_price_batch_fetcher,
            config,
        ));

        check_batching_many(buffered, tokens_requested).await;
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
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
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
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });

        let config = Configuration {
            max_concurrent_requests: NonZeroUsize::new(2),
            max_batch_len: 20,
            batch_delay: Duration::from_millis(50),
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
