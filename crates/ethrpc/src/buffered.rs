//! A buffered `Transport` implementation that automatically groups JSON RPC
//! requests into batches.

use {
    super::MAX_BATCH_SIZE,
    ethcontract::{
        jsonrpc::Call,
        web3::{BatchTransport, Error as Web3Error, RequestId, Transport},
    },
    futures::{
        channel::{mpsc, oneshot},
        future::{self, BoxFuture, FutureExt as _},
        stream::{self, FusedStream, Stream, StreamExt as _},
    },
    serde_json::Value,
    std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        future::Future,
        num::NonZeroUsize,
        sync::Arc,
        time::Duration,
    },
    tokio::task::JoinHandle,
    tracing::Instrument as _,
};

/// Buffered transport configuration.
pub struct Configuration {
    /// The maximum amount of concurrent batches to send to the node.
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

impl Default for Configuration {
    fn default() -> Self {
        // Default configuration behaves kind of like TCP Nagle.
        Self {
            max_concurrent_requests: NonZeroUsize::new(1),
            max_batch_len: MAX_BATCH_SIZE,
            batch_delay: Duration::default(),
        }
    }
}

/// Buffered `Transport` implementation that implements automatic batching of
/// JSONRPC requests.
#[derive(Clone, Debug)]
pub struct BufferedTransport<Inner> {
    inner: Arc<Inner>,
    calls: mpsc::UnboundedSender<CallContext>,
}

type RpcResult = Result<Value, Web3Error>;

type CallContext = (RequestId, Call, Option<String>, oneshot::Sender<RpcResult>);

impl<Inner> BufferedTransport<Inner>
where
    Inner: BatchTransport + Send + Sync + 'static,
    Inner::Out: Send,
    Inner::Batch: Send,
{
    /// Create a new buffered transport with the default configuration.
    pub fn new(inner: Inner) -> Self {
        Self::with_config(inner, Default::default())
    }

    /// Creates a new buffered transport with the specified configuration.
    pub fn with_config(inner: Inner, config: Configuration) -> Self {
        let inner = Arc::new(inner);
        let (calls, receiver) = mpsc::unbounded();
        Self::background_worker(inner.clone(), config, receiver);

        Self { inner, calls }
    }

    /// Start a background worker for handling batched requests.
    fn background_worker(
        inner: Arc<Inner>,
        config: Configuration,
        calls: mpsc::UnboundedReceiver<CallContext>,
    ) -> JoinHandle<()> {
        tokio::task::spawn(batched_for_each(config, calls, move |batch| {
            let inner = inner.clone();
            async move {
                let (mut requests, mut trace_ids, mut senders): (Vec<_>, Vec<_>, Vec<_>) =
                    itertools::multiunzip(
                        batch
                            .into_iter()
                            .filter(|(_, _, _, sender)| !sender.is_canceled())
                            .map(|(id, request, trace_id, sender)| {
                                ((id, request), trace_id, sender)
                            }),
                    );
                match requests.len() {
                    0 => (),
                    1 => {
                        let ((id, request), trace_id, sender) =
                            (requests.remove(0), trace_ids.remove(0), senders.remove(0));
                        let result = match (&request, trace_id) {
                            (Call::MethodCall(_), Some(trace_id)) => {
                                observe::request_id::set_task_local_storage(
                                    trace_id,
                                    inner.send(id, request),
                                )
                                .await
                            }
                            _ => inner.send(id, request).await,
                        };
                        let _ = sender.send(result);
                    }
                    n => {
                        // Group requests by trace_id(sorted), then group values by method
                        // name(unsorted) with call idx values(sorted).
                        let mut result_map: BTreeMap<String, HashMap<String, BTreeSet<usize>>> =
                            BTreeMap::new();
                        for (idx, ((_, call), trace_id)) in
                            requests.iter().zip(trace_ids).enumerate()
                        {
                            if let Call::MethodCall(call) = call {
                                let trace_id = trace_id.unwrap_or("-".to_string());
                                let method_name = call.method.clone();
                                result_map
                                    .entry(trace_id)
                                    .or_default()
                                    .entry(method_name)
                                    .or_default()
                                    .insert(idx);
                            }
                        }
                        // Produces the following format:
                        //  `1001:eth_call(0,2),eth_sendTransaction(4)|1002:eth_call(1,4),
                        // eth_sendTransaction(5)|-eth_call:(6,7,8,9,10)`
                        let request_metadata = result_map
                            .iter()
                            .map(|(trace_id, methods)| {
                                format!(
                                    "{}:{}",
                                    trace_id,
                                    methods
                                        .iter()
                                        .map(|(method, indices)| format!(
                                            "{}({})",
                                            method,
                                            indices
                                                .iter()
                                                .map(usize::to_string)
                                                .collect::<Vec<_>>()
                                                .join(",")
                                        ))
                                        .collect::<Vec<_>>()
                                        .join(",")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("|");

                        let results = observe::request_id::set_task_local_storage(
                            request_metadata,
                            inner.send_batch(requests),
                        )
                        .await
                        .unwrap_or_else(|err| vec![Err(err); n]);
                        for (sender, result) in senders.into_iter().zip(results) {
                            let _ = sender.send(result);
                        }
                    }
                }
            }
        }))
    }

    /// Queue a call by sending it over calls channel to the background worker.
    fn queue_call(&self, id: RequestId, request: Call) -> oneshot::Receiver<RpcResult> {
        let (sender, receiver) = oneshot::channel();
        let trace_id = observe::request_id::get_task_local_storage();
        let context = (id, request, trace_id, sender);
        self.calls
            .unbounded_send(context)
            .expect("worker task unexpectedly dropped");
        receiver
    }

    /// Executes a call.
    async fn execute_call(&self, id: RequestId, request: Call) -> RpcResult {
        let method = match &request {
            Call::MethodCall(call) => call.method.as_str(),
            _ => "none",
        };

        tracing::trace!(%id, %method, "queueing call");

        let response = self.queue_call(id, request);
        let result = response.await.expect("worker task unexpectedly dropped");

        tracing::trace!(%id, ok = %result.is_ok(), "received response");

        result
    }
}

impl<Inner> Transport for BufferedTransport<Inner>
where
    Inner: BatchTransport + Send + Sync + 'static,
    Inner::Out: Send,
    Inner::Batch: Send,
{
    type Out = BoxFuture<'static, RpcResult>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        self.inner.prepare(method, params)
    }

    fn send(&self, id: RequestId, request: Call) -> Self::Out {
        let this = self.clone();

        async move { this.execute_call(id, request).await }
            .in_current_span()
            .boxed()
    }
}

impl<Inner> BatchTransport for BufferedTransport<Inner>
where
    Inner: BatchTransport + Send + Sync + 'static,
    Inner::Out: Send,
    Inner::Batch: Send,
{
    type Batch = BoxFuture<'static, Result<Vec<RpcResult>, Web3Error>>;

    fn send_batch<T>(&self, requests: T) -> Self::Batch
    where
        T: IntoIterator<Item = (RequestId, Call)>,
    {
        let this = self.clone();
        let requests = requests.into_iter().collect::<Vec<_>>();

        async move {
            let responses = requests
                .into_iter()
                .map(|(id, request)| this.execute_call(id, request));
            Ok(future::join_all(responses).await)
        }
        .in_current_span()
        .boxed()
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
        crate::mock::MockTransport,
        ethcontract::{Web3, U256},
        mockall::predicate,
        serde_json::json,
    };

    #[tokio::test]
    async fn batches_calls_when_joining() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute_batch()
            .with(predicate::eq(vec![
                ("foo".to_owned(), vec![json!(true), json!("stuff")]),
                ("bar".to_owned(), vec![json!(42), json!("answer")]),
            ]))
            .returning(|_| Ok(vec![Ok(json!("hello")), Ok(json!("world"))]));

        let transport = BufferedTransport::new(transport);

        let (foo, bar) = futures::join!(
            transport.execute("foo", vec![json!(true), json!("stuff")]),
            transport.execute("bar", vec![json!(42), json!("answer")]),
        );
        assert_eq!(foo.unwrap(), json!("hello"));
        assert_eq!(bar.unwrap(), json!("world"));
    }

    #[tokio::test]
    async fn no_batching_with_only_one_request() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute()
            .with(
                predicate::eq("single".to_owned()),
                predicate::eq(vec![json!("request")]),
            )
            .returning(|_, _| Ok(json!(42)));

        let transport = BufferedTransport::new(transport);

        let response = transport
            .execute("single", vec![json!("request")])
            .await
            .unwrap();
        assert_eq!(response, json!(42));
    }

    #[tokio::test]
    async fn batches_separate_web3_instances() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute_batch()
            .with(predicate::eq(vec![
                ("eth_chainId".to_owned(), vec![]),
                ("eth_chainId".to_owned(), vec![]),
                ("eth_chainId".to_owned(), vec![]),
            ]))
            .returning(|_| {
                Ok(vec![
                    Ok(json!("0x2a")),
                    Ok(json!("0x2a")),
                    Ok(json!("0x2a")),
                ])
            });

        let web3 = Web3::new(BufferedTransport::new(transport));

        let chain_ids = future::try_join_all(vec![
            web3.clone().eth().chain_id(),
            web3.clone().eth().chain_id(),
            web3.clone().eth().chain_id(),
        ])
        .await
        .unwrap();

        assert_eq!(chain_ids, vec![U256::from(42); 3]);
    }

    #[tokio::test]
    async fn resolves_call_after_dropping_transport() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute()
            .with(predicate::eq("used".to_owned()), predicate::eq(vec![]))
            .returning(|_, _| Ok(json!(1337)));

        let transport = BufferedTransport::new(transport);

        let unused = transport.execute("unused", vec![]);
        let unpolled = transport.execute("unpolled", vec![]);
        let used = transport.execute("used", vec![]);
        drop((unused, transport));

        assert_eq!(used.await.unwrap(), json!(1337));
        drop(unpolled);
    }
}
