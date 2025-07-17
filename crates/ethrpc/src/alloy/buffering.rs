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
        transports::{TransportError, TransportErrorKind},
    },
    futures::{
        channel::{mpsc, oneshot},
        stream::StreamExt as _,
    },
    std::{
        collections::HashMap,
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

                    async move {
                        // map request_id => sender to quickly find the correct sender if
                        // the node returns sub-responses in a different order
                        let (mut senders, requests): (HashMap<_, _>, Vec<_>) = batch
                            .into_iter()
                            .filter(|(sender, _)| !sender.is_canceled())
                            .map(|(sender, request)| ((request.id().clone(), sender), request))
                            .unzip();
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
                                    let _ = sender.send(Err(TransportErrorKind::custom_str(&err)));
                                }
                            }
                            Ok(responses) => {
                                for response in responses {
                                    let Some(sender) = senders.remove(&response.id) else {
                                        tracing::warn!(id = ?response.id, "missing response for id");
                                        continue;
                                    };
                                    let _ = sender.send(Ok(response));
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
