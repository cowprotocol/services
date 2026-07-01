//! Transport layer that annotates outgoing HTTP RPC requests with headers
//! describing the JSON-RPC call(s) they carry: the method(s), the request
//! id(s), and the distributed-tracing request id.
//!
//! It MUST be installed as the innermost layer (closest to the HTTP transpor)
//! so that it observes the request packet exactly as it goes out on the wire.
use {
    alloy_json_rpc::{RequestPacket, SerializedRequest},
    reqwest::header::{HeaderName, HeaderValue},
    std::task::{Context, Poll},
    tower::{Layer, Service},
};

/// Comma-separated list of the JSON-RPC method(s) in the packet.
const METHOD: &str = "x-rpc-method";
/// Comma-separated list of the JSON-RPC id(s) in the packet.
const REQUEST_ID: &str = "x-rpc-request-id";
/// Distributed-tracing request id, correlating this call with the originating
/// task's logs across processes.
const TRACING_REQUEST_ID: &str = "x-request-id";

pub(crate) struct RpcHeadersLayer;

impl<S> Layer<S> for RpcHeadersLayer {
    type Service = RpcHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RpcHeadersService { inner }
    }
}

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
        let request_id = observe::tracing::distributed::request_id::from_current_span();
        annotate(&mut req, request_id.as_deref());
        self.inner.call(req)
    }
}

/// Attaches the RPC correlation headers to the packet.
///
/// The batching layer may coalesce several calls into one packet, so the method
/// and id headers list every call. The headers are written onto the last
/// request because header aggregation across a packet is last-wins, so a single
/// writer surfaces them for the whole request.
fn annotate(req: &mut RequestPacket, request_id: Option<&str>) {
    let requests = req.requests_mut();
    let methods = requests
        .iter()
        .map(SerializedRequest::method)
        .collect::<Vec<_>>()
        .join(",");
    let ids = requests
        .iter()
        .map(|r| r.id().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let Some(last) = requests.last_mut() else {
        return;
    };
    let headers = last.headers_mut();
    if let Some(v) = header_value(&methods) {
        headers.insert(HeaderName::from_static(METHOD), v);
    }
    if let Some(v) = header_value(&ids) {
        headers.insert(HeaderName::from_static(REQUEST_ID), v);
    }
    if let Some(v) = request_id.and_then(header_value) {
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
        alloy_json_rpc::{Id, Request},
    };

    fn request(method: &'static str, id: u64) -> SerializedRequest {
        Request::new(method, Id::Number(id), ())
            .serialize()
            .expect("serialize request")
    }

    #[test]
    fn single_call_lists_method_and_id() {
        let mut packet = RequestPacket::Single(request("eth_sendRawTransaction", 7));
        // No tracing request id in scope, so `x-request-id` should be absent.
        annotate(&mut packet, None);

        let headers = packet.headers();
        assert_eq!(headers[METHOD], "eth_sendRawTransaction");
        assert_eq!(headers[REQUEST_ID], "7");
        assert!(!headers.contains_key(TRACING_REQUEST_ID));
    }

    #[test]
    fn always_forwards_tracing_request_id() {
        let mut single = RequestPacket::Single(request("eth_call", 1));
        annotate(&mut single, Some("auction-42"));
        assert_eq!(single.headers()[TRACING_REQUEST_ID], "auction-42");

        let mut batch =
            RequestPacket::Batch(vec![request("eth_call", 1), request("eth_getBalance", 2)]);
        annotate(&mut batch, Some("auction-42"));
        assert_eq!(batch.headers()[TRACING_REQUEST_ID], "auction-42");
    }

    #[test]
    fn batch_lists_all_methods_and_ids() {
        let mut packet = RequestPacket::Batch(vec![
            request("eth_call", 1),
            request("eth_getBalance", 2),
            request("eth_chainId", 3),
        ]);
        annotate(&mut packet, Some("auction-42"));

        let headers = packet.headers();
        assert_eq!(headers[METHOD], "eth_call,eth_getBalance,eth_chainId");
        assert_eq!(headers[REQUEST_ID], "1,2,3");
        assert_eq!(headers[TRACING_REQUEST_ID], "auction-42");
    }

    #[test]
    fn batch_of_one_lists_single_method() {
        let mut packet = RequestPacket::Batch(vec![request("eth_chainId", 9)]);
        annotate(&mut packet, None);

        let headers = packet.headers();
        assert_eq!(headers[METHOD], "eth_chainId");
        assert_eq!(headers[REQUEST_ID], "9");
    }
}
