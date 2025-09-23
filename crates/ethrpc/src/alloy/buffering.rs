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

enum BatchRequestEntry {
    /// The entry is empty, this value should only be used for default-like
    /// initialization.
    Empty,
    Unique(ResponseSender),
    Duplicated(VecDeque<ResponseSender>),
}

impl BatchRequestEntry {
    /// Pushes a sender to the back of the entry, if the entry is `Unique` it
    /// will become `Duplicated`, if it is `Duplicated` it will just add it
    /// to the existing deque.
    fn push_back(&mut self, sender: ResponseSender) {
        let self_ = match std::mem::replace(self, Self::Empty) {
            BatchRequestEntry::Unique(old_sender) => {
                let mut deque = VecDeque::new();
                deque.push_back(old_sender);
                deque.push_back(sender);
                Self::Duplicated(deque)
            }
            BatchRequestEntry::Duplicated(mut senders) => {
                senders.push_back(sender);
                Self::Duplicated(senders)
            }
            BatchRequestEntry::Empty => Self::Unique(sender),
        };
        let _ = std::mem::replace(self, self_);
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
        tokio::task::spawn(
            calls
                .chunks_timeout(config.ethrpc_max_batch_size, config.ethrpc_batch_delay)
                .for_each_concurrent(config.ethrpc_max_concurrent_requests, move |batch|{
                    // Clones service via [`std::mem::replace`] as recommended by
                    // <https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services>
                    let clone: S = inner.clone();
                    let mut inner = std::mem::replace(&mut inner, clone);

                    // Map<Id, Senders> because even with random IDs we might get duplicates,
                    // (e.g. some ID outgrew another and now they overlap) in that case
                    // we use the Deque to enforce FIFO and hope the node didn't re-order responses
                    let mut senders: HashMap<_, BatchRequestEntry> = HashMap::with_capacity(batch.len());
                    let mut requests = Vec::with_capacity(batch.len());

                    async move {
                        for (sender, request) in batch {
                            if sender.is_canceled() {
                                tracing::trace!(request_id = %request.id(), "canceled sender");
                                continue
                            }
                            senders.entry(request.id().clone())
                            .or_insert(BatchRequestEntry::Empty)
                            .push_back(sender);
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
                            Err(err) => {
                                let err = format!("batch call failed: {err:?}");
                                for sender in senders.into_values() {
                                    match sender {
                                        BatchRequestEntry::Empty => {
                                            tracing::warn!("found empty batch entry, some sender might have been lost!");
                                        },
                                        BatchRequestEntry::Unique(sender) => {
                                            let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                                        },
                                        BatchRequestEntry::Duplicated(senders) => {
                                            for sender in senders {
                                                let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                                            }
                                        },
                                    }
                                }
                            }
                            Ok(responses) => {
                                for response in responses {
                                    tracing::trace!(response_id = %response.id, "attempting to remove response");

                                    let Some(entry) = senders.remove(&response.id) else {
                                        tracing::warn!(response_id = ?response.id, "missing sender for response");
                                        continue;
                                    };

                                    match entry {
                                        BatchRequestEntry::Empty => {
                                            tracing::warn!("found empty batch entry, some sender might have been lost!");
                                        },
                                        BatchRequestEntry::Unique(sender) => {
                                            let _ = sender.send(Ok(response));
                                        },
                                        BatchRequestEntry::Duplicated(mut response_senders) => {
                                            let response_id = response.id.clone();
                                            let Some(sender) = response_senders.pop_front() else {
                                                tracing::warn!(response_id = ?response.id, "received more responses than expected");
                                                continue;
                                            };
                                            let _ = sender.send(Ok(response));
                                            if !response_senders.is_empty() {
                                                senders.insert(response_id, BatchRequestEntry::Duplicated(response_senders));
                                            }
                                        },
                                    }
                                }
                            }
                        }
            }
        }))
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
    use {
        crate::alloy::buffering::BatchRequestEntry,
        futures::channel::oneshot,
        std::collections::VecDeque,
    };

    #[test]
    fn test_batch_entry_push_back() {
        let (tx_1, _) = oneshot::channel();
        let mut unique = BatchRequestEntry::Unique(tx_1);

        let (tx_2, _) = oneshot::channel();
        unique.push_back(tx_2);

        match unique {
            BatchRequestEntry::Unique(_) => panic!("failed to upgrade batch entry"),
            BatchRequestEntry::Duplicated(senders) => {
                assert_eq!(senders.len(), 2);
            }
            BatchRequestEntry::Empty => panic!("this entry shouldn't be empty for this test"),
        }
    }

    #[test]
    fn test_batch_entry_empty_to_unique() {
        let mut empty = BatchRequestEntry::Empty;

        let (tx, _) = oneshot::channel();
        empty.push_back(tx);

        match empty {
            BatchRequestEntry::Unique(_) => {
                // Success: Empty correctly upgraded to Unique
            }
            BatchRequestEntry::Duplicated(_) => {
                panic!("empty should upgrade to unique, not duplicated")
            }
            BatchRequestEntry::Empty => panic!("entry should no longer be empty"),
        }
    }

    #[test]
    fn test_batch_entry_duplicated_to_duplicated() {
        let (tx_1, _) = oneshot::channel();
        let (tx_2, _) = oneshot::channel();
        let mut deque = VecDeque::new();
        deque.push_back(tx_1);
        deque.push_back(tx_2);
        let mut duplicated = BatchRequestEntry::Duplicated(deque);

        let (tx_3, _) = oneshot::channel();
        duplicated.push_back(tx_3);

        match duplicated {
            BatchRequestEntry::Duplicated(senders) => {
                assert_eq!(senders.len(), 3);
            }
            BatchRequestEntry::Unique(_) => panic!("duplicated should remain duplicated"),
            BatchRequestEntry::Empty => panic!("duplicated should not become empty"),
        }
    }
}
