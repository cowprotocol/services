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
use {
    crate::Config,
    alloy::{
        rpc::json_rpc::{RequestPacket, Response, ResponsePacket, SerializedRequest},
        transports::{RpcError, TransportError, TransportErrorKind},
    },
    futures::{
        channel::{mpsc, oneshot},
        stream::StreamExt as _,
    },
    std::{
        collections::{HashMap, VecDeque},
        fmt::Debug,
        marker::PhantomData,
        pin::Pin,
        task::{Context, Poll},
    },
    tokio::task::JoinHandle,
    tokio_stream::StreamExt,
    tower::{Layer, Service},
};

/// Layer that buffers multiple calls into batch calls.
#[allow(dead_code)]
pub(crate) struct BatchCallLayer {
    config: Config,
}

impl BatchCallLayer {
    #[allow(dead_code)]
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
    calls: mpsc::UnboundedSender<CallContext>,
}

type CallContext = (
    oneshot::Sender<Result<Response, TransportError>>,
    SerializedRequest,
);

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
}

impl IntoIterator for BatchRequestEntry {
    type IntoIter = std::iter::Chain<
        std::option::IntoIter<ResponseSender>,
        std::collections::vec_deque::IntoIter<ResponseSender>,
    >;
    type Item = ResponseSender;

    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter().chain(self.duplicates)
    }
}

/// Data structure that assume that the node returns
/// the responses in the expected order. If that is not
/// the case only then will it pay the extra cost of
/// building a hashmap for faster lookup of unsorted
/// responses.
enum SenderLookUp<Id> {
    /// Variant that is used until the first response was delivered
    /// out of order. The expectation is that the next sender that
    /// needs to be retrieved is at the front.
    Sorted(VecDeque<(Id, ResponseSender)>),
    /// Variant that is used once we detect the first response that
    /// was delivered out of order.
    Unsorted(HashMap<Id, BatchRequestEntry>),
}

impl <Id> SenderLookUp<Id> where Id: Eq + PartialEq + std::hash::Hash {
    fn new(senders: VecDeque<(Id, ResponseSender)>) -> Self {
        Self::Sorted(senders)
    }

    /// Removes the next sender needed to report the response for the
    /// passed in request_id. Note that under some conditions the same
    /// batch might contain RPC calls with the same id. In that case
    /// you can also `.remove()` as many response senders for the
    /// duplicated id.
    fn remove(&mut self, request_id: &Id) -> Option<ResponseSender> {
        match self {
            Self::Sorted(senders) => {
                let (id, sender) = senders.pop_front()?;
                if *request_id == id {
                    // the order of returned requests still matches our expectations.
                    return Some(sender);
                }

                // The node returned the responses in an unexpected order so
                // we now have to build a hash table to quickly look up the
                // senders for all the incoming out of orders responses.
                tracing::debug!("build look up table for handling out of order batch response");
                let mut lookup = HashMap::with_capacity(senders.len() + 1);
                Self::insert_sender(&mut lookup, id, sender);

                let senders = std::mem::take(senders);
                for (id, sender) in senders {
                    Self::insert_sender(&mut lookup, id, sender);
                }

                // retrieve sender from data structure that's optimized for random retrieval,
                // convert Self to the new variant and return the sender
                let sender = lookup.get_mut(request_id).and_then(|senders| senders.pop_front());
                *self = Self::Unsorted(lookup);
                sender
            },
            Self::Unsorted(lookup) => {
                lookup.get_mut(request_id).and_then(|senders| senders.pop_front())
            }
        }
    }

    fn insert_sender(lookup: &mut HashMap<Id, BatchRequestEntry>, id: Id, sender: ResponseSender) {
        match lookup.entry(id) {
            std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().push_back(sender);
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(BatchRequestEntry::new(sender));
            }
        };
    }

    fn for_each(self, function: impl Fn(ResponseSender)) {
        match self {
            SenderLookUp::Sorted(senders) => {
                senders.into_iter().for_each(|(_id, sender)| {
                    function(sender);
                });
            },
            SenderLookUp::Unsorted(senders) => {
                senders.into_values().flatten().for_each(|sender| {
                    function(sender);
                });
            }
        }
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
        let (sender, receiver) = oneshot::channel();
        // Theoreticallly we could propagate the error to the caller, however
        // this is a critical error we can't recover from (i.e. we'll not be
        // able to send any more RPC calls). That's why we panic ASAP to immediately
        // cause a restart of the pod if this is running in kubernetes.
        self.calls
            .unbounded_send((sender, request))
            .expect("worker task unexpectedly dropped");
        receiver
    }

    /// Start a background worker for batching buffered requests.
    fn background_worker(
        mut inner: S,
        config: Config,
        calls: mpsc::UnboundedReceiver<CallContext>,
    ) -> JoinHandle<()> {
        let process_batch = move |batch: Vec<(ResponseSender, SerializedRequest)>| {
            // Clones service via [`std::mem::replace`] as recommended by
            // <https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services>
            let clone: S = inner.clone();
            let mut inner = std::mem::replace(&mut inner, clone);

            async move {
                // We use this weird mix of `into_iter().collect()` and `.push_back` in the
                // `.filter()` in order to make the compiler reuse the allocation of `batch`
                // for `requests`.
                let mut senders = VecDeque::with_capacity(batch.len());
                let requests: Vec<_> = batch
                    .into_iter()
                    .filter_map(|(sender, request)| {
                        if sender.is_canceled() {
                            tracing::trace!(request_id = %request.id(), "canceled sender");
                            return None;
                        }
                        senders.push_back((request.id().clone(), sender));
                        Some(request)
                    })
                    .collect();

                if requests.is_empty() {
                    tracing::trace!("all callers stopped awaiting their request");
                    return;
                }
                let mut senders = SenderLookUp::new(senders);

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
                            let Some(sender) = senders.remove(&response.id) else {
                                tracing::error!(id = ?response.id, "either unexpected response or no sender remaining for response");
                                continue;
                            };
                            tracing::trace!(response_id = %response.id, "sending response");
                            let _ = sender.send(Ok(response));
                        }

                        // if we still have senders left over we need to report an error so
                        // that they don't wait forever for a response that will never come
                        let err = format!("did not receive a response for the call");
                        senders.for_each(|sender| {
                            let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                        });
                    }
                    Err(err) => {
                        let err = format!("batch call failed: {err:?}");
                        senders.for_each(|sender| {
                            let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                        });
                    }
                }
            }
        };
        tokio::task::spawn(
            calls
                .chunks_timeout(config.ethrpc_max_batch_size, config.ethrpc_batch_delay)
                .for_each_concurrent(config.ethrpc_max_concurrent_requests, process_batch),
        )
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
    use {crate::alloy::buffering::BatchRequestEntry, futures::channel::oneshot};

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
}
