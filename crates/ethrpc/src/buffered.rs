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
        collections::{BTreeMap, BTreeSet},
        fmt::Write,
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
                                let span = observe::request_id::info_span(trace_id);
                                inner.send(id, request).instrument(span).await
                            }
                            _ => inner.send(id, request).await,
                        };
                        let _ = sender.send(result);
                    }
                    n => {
                        let results = match build_rpc_metadata(&requests, &trace_ids) {
                            Ok(metadata) => {
                                let span = observe::request_id::info_span(metadata);
                                inner.send_batch(requests).instrument(span).await
                            }
                            Err(err) => {
                                tracing::error!(
                                    ?err,
                                    "failed to build metadata, sending RPC calls without the \
                                     metadata header"
                                );
                                inner.send_batch(requests).await
                            }
                        }
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
        let trace_id = observe::request_id::from_current_span();
        // tracing::error!(trace_id, "enqueue call");
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

/// Builds a metadata string representation for RPC requests.
///
/// This function takes an iterator of requests and their corresponding trace
/// IDs, and generates a metadata string that groups the requests by their trace
/// IDs and method names. The format of the output string is as follows:
///
/// `trace_id:method_name(index1,index2,...),method_name(index1,index2,...
/// )|trace_id:...`
///
/// Each trace ID is followed by a colon and a list of method names. Each method
/// name is followed by a list of indices (representing the position of the
/// request in the original vector) enclosed in parentheses. Different method
/// names are separated by commas. If there are multiple trace IDs, their
/// entries are separated by a pipe character.
///
/// If a trace ID is `None`, it is represented as "null" in the output string.
/// All requests with absent trace IDs are grouped together under "null".
///
/// # Arguments
///
/// * `requests` - A vector of tuples, where each tuple contains a request ID
///   and a `Call` object representing the RPC request.
/// * `trace_ids` - A vector of optional strings representing the trace IDs of
///   the requests. The trace IDs correspond to the requests in the same
///   position in the `requests` vector.
///
/// # Returns
///
/// This function returns a string representing the metadata header.
fn build_rpc_metadata(
    requests: &[(RequestId, Call)],
    trace_ids: &[Option<String>],
) -> anyhow::Result<String> {
    // Group the requests by trace ID(sorted) and method name(sorted) where values
    // are sorted indices.
    let mut grouped_metadata: BTreeMap<String, BTreeMap<String, BTreeSet<usize>>> = BTreeMap::new();
    for (idx, ((_, call), trace_id)) in requests.iter().zip(trace_ids).enumerate() {
        if let Call::MethodCall(call) = call {
            let trace_id = trace_id.clone().unwrap_or("null".to_string());
            grouped_metadata
                .entry(trace_id)
                .or_default()
                .entry(call.method.clone())
                .or_default()
                .insert(idx);
        }
    }

    let mut metadata_str = String::new();

    let mut grouped_metadata_iter = grouped_metadata.into_iter().peekable();
    while let Some((trace_id, methods)) = grouped_metadata_iter.next() {
        // New entry starts with the trace_id
        write!(metadata_str, "{}:", trace_id)?;

        // Followed by the method names and their indices
        let mut methods_iter = methods.into_iter().peekable();
        while let Some((method, indices)) = methods_iter.next() {
            write!(metadata_str, "{}(", method)?;

            let indices_str = format_indices_as_ranges(indices)?;
            write!(metadata_str, "{}", indices_str)?;

            write!(metadata_str, ")")?;

            if methods_iter.peek().is_some() {
                write!(metadata_str, ",")?;
            }
        }

        if grouped_metadata_iter.peek().is_some() {
            write!(metadata_str, "|")?;
        }
    }

    Ok(metadata_str)
}

/// Formats a set of indices as a string of ranges.
///
/// This function takes a set of indices and formats them as a string where
/// consecutive indices are represented as ranges. For example, the set
/// `{1, 2, 3, 5, 6, 8}` would be formatted as the string `"1..3,5..6,8"`.
///
/// # Arguments
///
/// * `indices` - A set of indices to format. The indices should be unique and
///   sorted in ascending order.
///
/// # Returns
///
/// This function returns a string representing the indices as ranges. Each
/// range is formatted as `start..end`, and ranges are separated by commas.
/// Single indices (i.e., indices that are not part of a range) are represented
/// as themselves.
fn format_indices_as_ranges(indices: BTreeSet<usize>) -> anyhow::Result<String> {
    let mut result = String::new();
    let mut indices = indices.into_iter();
    // Initialize the start and last variables with the first index.
    let mut start = match indices.next() {
        Some(index) => index,
        None => return Ok(result),
    };
    let mut last = start;

    // Iterate over the rest of the indices
    for index in indices {
        // If the current index is the next consecutive number, update last index.
        if index == last + 1 {
            last = index;
        // Otherwise, there is no need to accumulate the range anymore. Append
        // the range to the result string.
        } else {
            append_sequence(&mut result, start, last)?;
            write!(result, ",")?;
            // Reset the start and last indices with the current value.
            start = index;
            last = index;
        }
    }

    // Append the remaining data.
    append_sequence(&mut result, start, last)?;

    Ok(result)
}

/// This function formats a range of integers into a condensed string
/// representation and appends it to the given buffer. The format varies based
/// on the relationship between `start` and `last`:
///
/// - If `start` is equal to `last`, it indicates a single value, which is
///   appended as such.
/// - If `start` is one less than `last` (i.e., they are consecutive), both
///   numbers are appended separated by a comma.
/// - Otherwise, the numbers between `start` and `last` (inclusive) are
///   represented as a range using two dots (e.g., "start..last").
fn append_sequence(buffer: &mut String, start: usize, last: usize) -> core::fmt::Result {
    if start == last {
        write!(buffer, "{}", start)
    } else if start == last - 1 {
        write!(buffer, "{},{}", start, last)
    } else {
        write!(buffer, "{}..{}", start, last)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::mock::MockTransport,
        ethcontract::{
            jsonrpc::{Id, MethodCall, Params},
            Web3,
            U256,
        },
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

    #[test]
    fn test_format_indices_as_ranges() {
        // empty string
        let indices = BTreeSet::new();
        assert_eq!(format_indices_as_ranges(indices).unwrap(), "");

        // a single value
        let indices = vec![2].into_iter().collect();
        assert_eq!(format_indices_as_ranges(indices).unwrap(), "2");

        // only a range
        let indices = vec![1, 2, 3, 4, 5].into_iter().collect();
        assert_eq!(format_indices_as_ranges(indices).unwrap(), "1..5");

        // 2 subsequent values range
        let indices = vec![2, 3].into_iter().collect();
        assert_eq!(format_indices_as_ranges(indices).unwrap(), "2,3");

        // no ranges
        let indices = vec![1, 3, 5, 7].into_iter().collect();
        assert_eq!(format_indices_as_ranges(indices).unwrap(), "1,3,5,7");

        // ends with a non-range value
        let indices = vec![1, 2, 3, 5, 7, 8, 9, 10, 20].into_iter().collect();
        assert_eq!(
            format_indices_as_ranges(indices).unwrap(),
            "1..3,5,7..10,20"
        );

        // ends with a range value
        let indices = vec![1, 2, 3, 5, 6, 7, 8, 10, 11, 12].into_iter().collect();
        assert_eq!(
            format_indices_as_ranges(indices).unwrap(),
            "1..3,5..8,10..12"
        );
    }

    fn method_call(method: &str) -> Call {
        Call::MethodCall(MethodCall {
            jsonrpc: None,
            method: method.to_string(),
            params: Params::None,
            id: Id::Null,
        })
    }

    #[test]
    fn test_build_rpc_metadata_header() {
        let requests = vec![
            (1001, method_call("eth_sendTransaction")), // 0
            (1001, method_call("eth_call")),            // 1
            (1001, method_call("eth_sendTransaction")), // 2
            (1002, method_call("eth_call")),            // 3
            (9999, method_call("eth_call")),            // 4
            (1001, method_call("eth_sendTransaction")), // 5
            (1002, method_call("eth_call")),            // 6
            (1002, method_call("eth_call")),            // 7
            (1001, method_call("eth_sendTransaction")), // 8
            (9999, method_call("eth_sendTransaction")), // 9
            (9999, method_call("eth_sendTransaction")), // 10
            (9999, method_call("eth_sendTransaction")), // 11
        ];
        let trace_ids = vec![
            Some("1001".to_string()), // 0
            Some("1001".to_string()), // 1
            Some("1001".to_string()), // 2
            Some("1002".to_string()), // 3
            None,                     // 4
            Some("1001".to_string()), // 5
            Some("1002".to_string()), // 6
            Some("1002".to_string()), // 7
            Some("1001".to_string()), // 8
            None,                     // 9
            None,                     // 10
            None,                     // 11
        ];
        let metadata_header = build_rpc_metadata(&requests, &trace_ids).unwrap();
        assert_eq!(
            metadata_header,
            "1001:eth_call(1),eth_sendTransaction(0,2,5,8)|1002:eth_call(3,6,7)|null:eth_call(4),\
             eth_sendTransaction(9..11)"
        );
    }
}
