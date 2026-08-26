//! Implements the [`BatchCallLayer`] which automatically batches individual RPC
//! calls together. Batching happens via the node's [batching
//! feature](https://geth.ethereum.org/docs/interacting-with-geth/rpc/batch)
//! instead of alloy's native
//! [MultiCall3](https://docs.rs/alloy/latest/alloy/providers/layers/struct.CallBatchLayer.html)
//! based batching.
//!
//! To do achieve that the layer does not execute any requests itself.
//! Instead it sends the requests into a queue which a background task will
//! read from. The background task then does the batching, forwards the requests
//! to the next layer, and reports the results of the individual calls back via
//! another channel.
//!
//! To prevent a single caller from monopolizing the pipeline, each enqueued
//! call is tagged with the tokio task id of the caller. The background worker
//! keeps a queue per caller and assembles batches by round-robin across those
//! queues. Fairness is best-effort: it depends on each subsystem driving its
//! RPC calls from a stable task rather than spawning a fresh task per request.
use {
    crate::Config,
    alloy_json_rpc::{RequestPacket, Response, ResponsePacket, SerializedRequest},
    alloy_transport::{RpcError, TransportError, TransportErrorKind},
    futures::{
        channel::{
            mpsc::{self, TryRecvError},
            oneshot,
        },
        stream::StreamExt as _,
    },
    std::{
        collections::{HashMap, VecDeque, hash_map::Entry},
        fmt::Debug,
        marker::PhantomData,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    },
    tokio::{sync::Semaphore, task::JoinHandle},
    tower::{Layer, Service},
};

/// Layer that buffers multiple calls into batch calls.
pub(crate) struct BatchCallLayer {
    config: Config,
}

impl BatchCallLayer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl<S> Layer<S> for BatchCallLayer
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Clone
        + Sync
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    type Service = BatchCallProvider<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BatchCallProvider::new(self.config.clone(), inner)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BatchCallProvider<S> {
    inner: PhantomData<S>,
    calls: mpsc::UnboundedSender<CallContext<SerializedRequest, Result<Response, TransportError>>>,
}

/// Identifies which tokio task enqueued a call. `None` covers contexts
/// without a task id (e.g. calls issued from blocking tasks).
/// The tokio runtime reuses ids under certain conditions. In practice
/// we should not run into those cases but even if we do the worst thing
/// that can happen is that we think the old task sent more requests
/// which is not a huge issue.
type CallerId = Option<tokio::task::Id>;

struct CallContext<Req, Resp> {
    /// tokio task that issued this request
    caller: CallerId,
    request: Req,
    response_sender: oneshot::Sender<Resp>,
}

type ResponseSender = oneshot::Sender<Result<Response, RpcError<TransportErrorKind>>>;

/// Batch to keep track of duplicates in FIFO order.
/// Will spill *all* duplicate elements to `duplicates` instead of ever
/// back-filling the head.
///
/// The idea behind this approach is to avoid extra allocations and indirections
/// for non-duplicate items (most of them).
struct BatchRequestEntry {
    value: Option<ResponseSender>,
    duplicates: VecDeque<ResponseSender>,
}

impl BatchRequestEntry {
    fn new(sender: ResponseSender) -> Self {
        Self {
            value: Some(sender),
            duplicates: Default::default(),
        }
    }

    fn push_back(&mut self, sender: ResponseSender) {
        debug_assert!(
            self.value.is_some(),
            "cannot push_back after you start pop_front"
        );
        // Never puts anything in `value` because it would break the whole premise of
        // "pushing back"
        self.duplicates.push_back(sender);
    }

    fn pop_front(&mut self) -> Option<ResponseSender> {
        self.value.take().or_else(|| self.duplicates.pop_front())
    }

    fn into_iter(self) -> impl Iterator<Item = ResponseSender> {
        self.value.into_iter().chain(self.duplicates)
    }
}

impl<S> BatchCallProvider<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Clone
        + Sync
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    fn new(config: Config, inner: S) -> Self {
        let (calls, receiver) = mpsc::unbounded();
        let res = Self {
            calls,
            inner: PhantomData,
        };
        Self::background_worker(inner, config, receiver);
        res
    }

    /// Enqueues a call for execution by sending it to the background task.
    fn enqueue_call(
        &self,
        request: SerializedRequest,
    ) -> oneshot::Receiver<Result<Response, TransportError>> {
        let (response_sender, receiver) = oneshot::channel();
        // Tag with the caller's tokio task id so the worker can interleave
        // requests from different subsystems fairly.
        let caller = tokio::task::try_id();
        // Theoreticallly we could propagate the error to the caller, however
        // this is a critical error we can't recover from (i.e. we'll not be
        // able to send any more RPC calls). That's why we panic ASAP to immediately
        // cause a restart of the pod if this is running in kubernetes.
        self.calls
            .unbounded_send(CallContext {
                caller,
                request,
                response_sender,
            })
            .expect("worker task unexpectedly dropped");
        receiver
    }

    /// Start a background worker for batching buffered requests.
    ///
    /// The worker keeps one queue per caller (keyed by the tokio task id
    /// captured at enqueue time). Each batch is assembled by round-robin
    /// across those queues, which prevents a single caller from monopolizing
    /// the pipeline when many requests are enqueued in a burst.
    fn background_worker(
        mut inner: S,
        config: Config,
        mut calls: mpsc::UnboundedReceiver<
            CallContext<SerializedRequest, Result<Response, TransportError>>,
        >,
    ) -> JoinHandle<()> {
        let permits = if config.ethrpc_max_concurrent_requests == 0 {
            Semaphore::MAX_PERMITS
        } else {
            config.ethrpc_max_concurrent_requests
        };
        let semaphore = Arc::new(Semaphore::new(permits));
        let max_batch_size = config.ethrpc_max_batch_size;
        let batch_delay = config.ethrpc_batch_delay;

        tokio::task::spawn(async move {
            let mut queue =
                FairQueue::<SerializedRequest, Result<Response, TransportError>>::default();

            loop {
                // first wait for a concurrency slot to become available.
                // that way we end up with the most up-to-data batch
                // possible in the end.
                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("semaphore never closed");

                if !queue
                    .collect_requests(&mut calls, max_batch_size, batch_delay)
                    .await
                {
                    tracing::debug!("rpc batching channel closed");
                    return;
                }

                let batch = queue.build_fair_batch(max_batch_size);

                // Clone the inner service per batch as recommended in
                // <https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services>.
                let clone = inner.clone();
                let this_inner = std::mem::replace(&mut inner, clone);

                tokio::task::spawn(async move {
                    // move permit into the task so we only return it when
                    // task is done
                    let _permit = permit;
                    process_batch(this_inner, batch).await;
                });
            }
        })
    }
}

/// Fair FIFO queue of pending calls partitioned by caller. Items are
/// enqueued into per-caller sub-queues and dequeued round-robin, so a
/// single caller with a large backlog cannot monopolize the pipeline.
///
/// Invariant: a caller is in `round_robin` iff its entry in `per_caller`
/// is non-empty, and `len` equals the sum of all per-caller queue lengths.
struct FairQueue<Req, Resp> {
    per_caller: HashMap<CallerId, VecDeque<(Req, oneshot::Sender<Resp>)>>,
    round_robin: VecDeque<CallerId>,
    len: usize,
}

impl<Req, Resp> Default for FairQueue<Req, Resp> {
    fn default() -> Self {
        Self {
            per_caller: Default::default(),
            round_robin: Default::default(),
            len: 0,
        }
    }
}

impl<Req, Resp> FairQueue<Req, Resp> {
    /// Enqueues request and adds caller to the round-robin queue if necessary.
    fn enqueue(&mut self, call: CallContext<Req, Resp>) {
        let queue = self.per_caller.entry(call.caller).or_default();
        let first = queue.is_empty();
        queue.push_back((call.request, call.response_sender));
        if first {
            self.round_robin.push_back(call.caller);
        }
        self.len += 1;
    }

    /// Pop the next call in round-robin order, if any.
    fn pop(&mut self) -> Option<(Req, oneshot::Sender<Resp>)> {
        let caller = self.round_robin.pop_front()?;
        let queue = self
            .per_caller
            .get_mut(&caller)
            .expect("caller in round_robin has a non-empty per-caller queue");
        let item = queue
            .pop_front()
            .expect("caller in round_robin has a non-empty per-caller queue");
        if queue.is_empty() {
            self.per_caller.remove(&caller);
        } else {
            self.round_robin.push_back(caller);
        }
        self.len -= 1;
        Some(item)
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Drains the items from `calls` into the fair queue until we have at least
    /// `max_batch_size` items or waited for `batch_delay` after starting to
    /// build a batch.
    /// 1. drain all immediately available items from `calls`
    /// 2. if we still have no items, await the next one
    /// 3. if we don't have at least `max_batch_size` items yet, await for
    ///    `batch_delay` time longer (to avoid tiny batches)
    ///
    /// Returns `true` if the `calls` channels is still alive and continuing
    /// to process requests makes sense.
    async fn collect_requests(
        &mut self,
        calls: &mut mpsc::UnboundedReceiver<CallContext<Req, Resp>>,
        max_batch_size: usize,
        batch_delay: Duration,
    ) -> bool {
        loop {
            match calls.try_recv() {
                Ok(call) => self.enqueue(call),
                Err(TryRecvError::Closed) => {
                    return false;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        // wait for the next request to appear. do this outside the select
        // loop below to only start the timeout when we actually have an item
        // in the pipeline
        if self.is_empty() {
            let Some(call) = calls.next().await else {
                return false;
            };
            self.enqueue(call);
        }

        if self.len() < max_batch_size && !batch_delay.is_zero() {
            let deadline = tokio::time::sleep(batch_delay);
            tokio::pin!(deadline);
            while self.len() < max_batch_size {
                tokio::select! {
                    _ = &mut deadline => break,
                    msg = calls.next() => {
                        let Some(call) = msg else {
                            return false;
                        };
                        self.enqueue(call);
                    }
                }
            }
        }

        true
    }

    /// Batches at most `max_batch_size` items in a round-robin fashion to
    /// prevent individual callers from starving all the others.
    fn build_fair_batch(&mut self, max_batch_size: usize) -> Vec<(Req, oneshot::Sender<Resp>)> {
        let mut batch = Vec::with_capacity(self.len().min(max_batch_size));
        while batch.len() < max_batch_size {
            let Some((request, sender)) = self.pop() else {
                break;
            };
            if !sender.is_canceled() {
                // only add to batch if caller is still waiting for response
                batch.push((request, sender));
            }
        }
        batch
    }
}

async fn process_batch<S>(mut inner: S, batch: Vec<(SerializedRequest, ResponseSender)>)
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
{
    // Map<Id, Senders> because even with random IDs we might get duplicates,
    // (e.g. some ID outgrew another and now they overlap) in that case
    // we use the Deque to enforce FIFO and hope the node didn't re-order responses
    let mut senders: HashMap<_, BatchRequestEntry> = HashMap::with_capacity(batch.len());
    let mut requests = Vec::with_capacity(batch.len());

    for (request, sender) in batch {
        if sender.is_canceled() {
            tracing::trace!(request_id = %request.id(), "canceled sender");
            continue;
        }
        match senders.entry(request.id().clone()) {
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().push_back(sender);
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(BatchRequestEntry::new(sender));
            }
        }
        requests.push(request);
    }

    if requests.is_empty() {
        tracing::trace!("all callers stopped awaiting their request");
        return;
    }

    let result = inner
        .call(RequestPacket::Batch(requests))
        .await
        .map(|response| match response {
            ResponsePacket::Batch(res) => res,
            ResponsePacket::Single(res) => {
                tracing::warn!("received single response for batch request");
                vec![res]
            }
        });

    match result {
        Ok(responses) => {
            for response in responses {
                tracing::trace!(response_id = %response.id, "attempting to remove response");
                let Some(entry) = senders.get_mut(&response.id) else {
                    tracing::warn!(response_id = %response.id, "missing sender for response");
                    continue;
                };
                let Some(sender) = entry.pop_front() else {
                    tracing::warn!(response_id = %response.id, "more responses than senders (may have lost some sender)");
                    continue;
                };
                tracing::debug!(response_id = %response.id, "sending response");
                let _ = sender.send(Ok(response));
            }
        }
        Err(err) => {
            let err = format!("batch call failed: {err:?}");
            senders
                .into_values()
                .flat_map(|sender| sender.into_iter())
                .for_each(|sender| {
                    let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                });
        }
    }
}

impl<S> Service<RequestPacket> for BatchCallProvider<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Clone
        + Sync
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    type Response = S::Response;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.calls.is_closed() {
            Poll::Ready(Err(TransportErrorKind::custom_str(
                "background task for batching requests was dropped unexpectedly",
            )))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn call(&mut self, packet: RequestPacket) -> Self::Future {
        match packet {
            RequestPacket::Single(request) => {
                let response = self.enqueue_call(request);
                Box::pin(async move {
                    let response = response.await.map_err(|err| {
                        TransportErrorKind::custom_str(&format!(
                            "failed to receive response from batching layer background task: \
                             {err:?}"
                        ))
                    })??;
                    Ok(ResponsePacket::Single(response))
                })
            }
            // Mapping errors of these batch requests is very annoying and we
            // don't need manual batching anyway with this layer so we just
            // don't support it.
            RequestPacket::Batch(_) => Box::pin(async {
                Err(TransportErrorKind::custom_str(
                    "manually batching calls is not supported by the auto batching layer",
                ))
            }),
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, futures::FutureExt as _};

    #[test]
    fn test_batch_request_entry_pop_twice() {
        let (sender, _receiver) = oneshot::channel();
        let mut entry = BatchRequestEntry::new(sender);

        let first_pop = entry.pop_front();
        assert!(first_pop.is_some());

        let second_pop = entry.pop_front();
        assert!(second_pop.is_none());
    }

    #[test]
    fn test_batch_request_entry_add_element_pop_thrice() {
        let (sender1, _receiver1) = oneshot::channel();
        let (sender2, _receiver2) = oneshot::channel();
        let mut entry = BatchRequestEntry::new(sender1);

        entry.push_back(sender2);

        let first_pop = entry.pop_front();
        assert!(first_pop.is_some());

        let second_pop = entry.pop_front();
        assert!(second_pop.is_some());

        let third_pop = entry.pop_front();
        assert!(third_pop.is_none());
    }

    /// Tests that the fair queue builds batches in a round robin fashion.
    /// Also tests that the associated response senders send the data to
    /// the correct caller.
    #[tokio::test]
    async fn batching_does_round_robin() {
        let (request_sender, mut receiver) = mpsc::unbounded();
        let mut queue = FairQueue::default();

        fn call_context(index: u64) -> (CallContext<u64, u64>, oneshot::Receiver<u64>) {
            let (response_sender, receiver) = oneshot::channel();
            let context = CallContext {
                caller: tokio::task::try_id(),
                request: index,
                response_sender,
            };
            (context, receiver)
        }

        // spammy producer that enques 100 calls before other
        // tasks even start
        let mut response_receivers: Vec<_> = (0..100)
            .map(|id| {
                let (context, receiver) = call_context(id);
                request_sender.unbounded_send(context).unwrap();
                receiver
            })
            .collect();

        for id in 100..103 {
            let request_sender2 = request_sender.clone();

            // enqueue calls from new separate tasks to give each one
            // its own queue to test round robin (keyed by task id)
            #[allow(clippy::async_yields_async)]
            let receiver = tokio::task::spawn(async move {
                let (context, receiver) = call_context(id);
                request_sender2.unbounded_send(context).unwrap();
                receiver
            })
            .await
            .unwrap();
            response_receivers.push(receiver);
        }

        let should_continue = queue
            .collect_requests(&mut receiver, 5, Default::default())
            .now_or_never()
            .expect("if we have enough requests already enqueued this is actually sync");
        assert!(should_continue);

        let batch = queue.build_fair_batch(5);

        // ASSERT THAT BATCH WAS FAIR (ROUND ROBIN)
        assert_eq!(batch.len(), 5);
        let mut iter = batch.into_iter();
        // first request of spammy producer
        let (request, sender) = iter.next().unwrap();
        assert_eq!(request, 0);
        sender.send(0).unwrap();

        // requests of other producers
        let (request, sender) = iter.next().unwrap();
        assert_eq!(request, 100);
        sender.send(100).unwrap();
        let (request, sender) = iter.next().unwrap();
        assert_eq!(request, 101);
        sender.send(101).unwrap();
        let (request, sender) = iter.next().unwrap();
        assert_eq!(request, 102);
        sender.send(102).unwrap();

        // round robin wrapped around so this is the second request of the spammy
        // producer
        let (request, sender) = iter.next().unwrap();
        assert_eq!(request, 1);
        sender.send(1).unwrap();

        // ASSERT THAT RESPONSES REACHED THE CORRECT CALLERS
        let mut responses = response_receivers.into_iter();
        // first 2 calls of the spammy producer resolved
        assert_eq!(responses.next().unwrap().now_or_never().unwrap(), Ok(0));
        assert_eq!(responses.next().unwrap().now_or_never().unwrap(), Ok(1));
        // next 98 calls fo the spammy producer did not resolve yet
        for _ in 0..98 {
            assert!(responses.next().unwrap().now_or_never().is_none());
        }
        // requests of other producers resolved
        assert_eq!(responses.next().unwrap().now_or_never().unwrap(), Ok(100));
        assert_eq!(responses.next().unwrap().now_or_never().unwrap(), Ok(101));
        assert_eq!(responses.next().unwrap().now_or_never().unwrap(), Ok(102));
    }
}
