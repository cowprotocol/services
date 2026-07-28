//! Transport layers that annotate outgoing HTTP RPC requests with headers
//! describing the JSON-RPC call(s) they carry: the `method:id` pair(s) and the
//! distributed-tracing request id.

use {
    alloy_json_rpc::RequestPacket,
    reqwest::header::{HeaderName, HeaderValue},
    std::{
        fmt::Write as _,
        task::{Context, Poll},
    },
    tower::{Layer, Service},
};

/// Comma-separated list of the JSON-RPC calls in the packet, each formatted as
/// `method:id` so every method stays paired with its id.
const CALLS: &str = "x-rpc-calls";
/// Distributed-tracing request id, correlating this call with the originating
/// task's logs across processes.
const TRACING_REQUEST_ID: &str = "x-request-id";

/// MUST be installed as the innermost layer (closest to the HTTP transport)
/// so it observes the final (possibly batched) packet on the wire;
/// it writes the `method:id` list and aggregates the request ids stamped above.
pub(crate) struct RpcHeadersLayer;

impl<S> Layer<S> for RpcHeadersLayer {
    type Service = RpcHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RpcHeadersService { inner }
    }
}

/// Runs *outside* the batching layer, while still on the caller's task,
/// and stamps the current span's distributed-tracing request id onto the
/// request. This is necessary because the batching layer forwards requests
/// from a  background task that no longer sees that span.
#[derive(Debug, Clone)]
pub(crate) struct RpcHeadersService<S> {
    inner: S,
}

impl<S> Service<RequestPacket> for RpcHeadersService<S>
where
    S: Service<RequestPacket> + Send + 'static,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: RequestPacket) -> Self::Future {
        annotate(&mut req);
        self.inner.call(req)
    }
}

/// Stamps the current span's distributed-tracing request id onto the request as
/// a header.
///
/// This MUST run *outside* the batching layer. The batching layer forwards
/// requests to the transport from a background task that no longer has the
/// originating span in scope, so the id has to be captured here — while we are
/// still on the caller's task — and carried on the request itself.
pub(crate) struct TracingRequestIdLayer;

impl<S> Layer<S> for TracingRequestIdLayer {
    type Service = TracingRequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingRequestIdService { inner }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TracingRequestIdService<S> {
    inner: S,
}

impl<S> Service<RequestPacket> for TracingRequestIdService<S>
where
    S: Service<RequestPacket> + Send + 'static,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: RequestPacket) -> Self::Future {
        if let Some(request_id) = observe::tracing::distributed::request_id::from_current_span()
            .as_deref()
            .and_then(header_value)
        {
            for r in req.requests_mut() {
                r.headers_mut().insert(
                    HeaderName::from_static(TRACING_REQUEST_ID),
                    request_id.clone(),
                );
            }
        }
        self.inner.call(req)
    }
}

/// Attaches the RPC correlation headers to the packet.
///
/// A batched packet coalesces several calls, so the calls header lists every
/// `method:id` pair and the request-id header aggregates the distinct tracing
/// ids that [`TracingRequestIdLayer`] stamped on the individual sub-requests.
/// The packet's HTTP headers are the union of every sub-request's headers, so
/// both are consolidated onto a single sub-request (the last) — writing them on
/// more than one would send the header repeatedly.
fn annotate(req: &mut RequestPacket) {
    let requests = req.requests_mut();
    // Build the `method:id` list and aggregate the tracing request ids in a
    // single pass, writing directly into the buffers to avoid intermediate
    // `Vec` and per-item `String` allocations.
    let mut calls = String::new();
    let mut request_ids = String::new();
    for (i, r) in requests.iter().enumerate() {
        if i > 0 {
            calls.push(',');
        }
        let _ = write!(calls, "{}:{}", r.method(), r.id());

        if let Some(id) = r
            .headers()
            .and_then(|h| h.get(TRACING_REQUEST_ID))
            .and_then(|v| v.to_str().ok())
        {
            // A batch can merge several calls sharing one id; keep them distinct.
            if !request_ids.split(',').any(|seen| seen == id) {
                if !request_ids.is_empty() {
                    request_ids.push(',');
                }
                request_ids.push_str(id);
            }
        }
    }
    // The packet's HTTP headers are the union of every sub-request's headers, so
    // a header present on multiple sub-requests would be sent repeatedly. Strip
    // the per-sub-request tracing ids and re-emit the aggregate on the last
    // request only.
    for r in requests.iter_mut() {
        r.headers_mut().remove(TRACING_REQUEST_ID);
    }
    let Some(last) = requests.last_mut() else {
        return;
    };
    let headers = last.headers_mut();
    if let Some(v) = header_value(&calls) {
        headers.insert(HeaderName::from_static(CALLS), v);
    }
    if !request_ids.is_empty()
        && let Some(v) = header_value(&request_ids)
    {
        headers.insert(HeaderName::from_static(TRACING_REQUEST_ID), v);
    }
}

fn header_value(s: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(s).ok()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_json_rpc::{Id, Request, SerializedRequest},
    };

    fn request(method: &'static str, id: u64) -> SerializedRequest {
        Request::new(method, Id::Number(id), ())
            .serialize()
            .expect("serialize request")
    }

    /// Stamps the tracing request id header, mirroring what
    /// [`TracingRequestIdService`] does before the batching layer.
    fn with_request_id(mut req: SerializedRequest, id: &str) -> SerializedRequest {
        req.headers_mut().insert(
            HeaderName::from_static(TRACING_REQUEST_ID),
            HeaderValue::from_str(id).unwrap(),
        );
        req
    }

    #[test]
    fn single_call_lists_method_and_id() {
        let mut packet = RequestPacket::Single(request("eth_sendRawTransaction", 7));
        // No tracing request id stamped, so `x-request-id` should be absent.
        annotate(&mut packet);

        let headers = packet.headers();
        assert_eq!(headers[CALLS], "eth_sendRawTransaction:7");
        assert!(!headers.contains_key(TRACING_REQUEST_ID));
    }

    #[test]
    fn batch_lists_all_methods_and_ids() {
        let mut packet = RequestPacket::Batch(vec![
            request("eth_call", 1),
            request("eth_getBalance", 2),
            request("eth_chainId", 3),
        ]);
        annotate(&mut packet);

        assert_eq!(
            packet.headers()[CALLS],
            "eth_call:1,eth_getBalance:2,eth_chainId:3"
        );
    }

    #[test]
    fn batch_of_one_lists_single_method() {
        let mut packet = RequestPacket::Batch(vec![request("eth_chainId", 9)]);
        annotate(&mut packet);

        assert_eq!(packet.headers()[CALLS], "eth_chainId:9");
    }

    #[test]
    fn forwards_single_request_id() {
        let mut packet =
            RequestPacket::Single(with_request_id(request("eth_call", 1), "auction-42"));
        annotate(&mut packet);

        let headers = packet.headers();
        assert_eq!(headers[CALLS], "eth_call:1");
        assert_eq!(headers[TRACING_REQUEST_ID], "auction-42");
    }

    #[test]
    fn aggregates_distinct_tracing_request_ids() {
        // A batch merged across auctions carries several ids; they are all kept.
        let mut packet = RequestPacket::Batch(vec![
            with_request_id(request("eth_call", 1), "auction-1"),
            with_request_id(request("eth_getBalance", 2), "auction-2"),
        ]);
        annotate(&mut packet);

        assert_eq!(packet.headers()[TRACING_REQUEST_ID], "auction-1,auction-2");
    }

    #[test]
    fn deduplicates_shared_tracing_request_id() {
        // Sub-requests sharing one id collapse to a single value.
        let mut packet = RequestPacket::Batch(vec![
            with_request_id(request("eth_call", 1), "auction-42"),
            with_request_id(request("eth_getBalance", 2), "auction-42"),
        ]);
        annotate(&mut packet);

        assert_eq!(packet.headers()[TRACING_REQUEST_ID], "auction-42");
    }
}
