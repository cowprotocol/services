use crate::metrics::get_metrics_registry;
use ethcontract::jsonrpc as jsonrpc_core;
use futures::{future::BoxFuture, FutureExt};
use jsonrpc_core::types::{Call, Output, Request, Value};
use prometheus::Registry;
use reqwest::{Client, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use web3::{error::Error as Web3Error, helpers, BatchTransport, RequestId, Transport};

#[derive(Clone, Debug)]
pub struct HttpTransport {
    client: Client,
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    url: Url,
    id: AtomicUsize,
}

impl HttpTransport {
    pub fn new(client: Client, url: Url) -> Self {
        Self {
            client,
            inner: Arc::new(Inner {
                url,
                id: AtomicUsize::new(0),
            }),
        }
    }

    pub fn next_id(&self) -> RequestId {
        self.inner.id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn new_request(&self) -> (Client, Url) {
        (self.client.clone(), self.inner.url.clone())
    }
}

// Id is only used for logging.
async fn execute_rpc<T: DeserializeOwned>(
    client: Client,
    url: Url,
    id: RequestId,
    request: &Request,
) -> Result<T, Web3Error> {
    tracing::debug!(
        "[id:{}] sending request: {:?}",
        id,
        serde_json::to_string(&request)?
    );
    let response = client.post(url).json(request).send().await.map_err(|err| {
        let message = format!("failed to send request: {}", err);
        tracing::debug!("[id:{}] {}", id, message);
        Web3Error::Transport(message)
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|err| {
        let message = format!("failed to get response body: {}", err);
        tracing::debug!("[id:{}] {}", id, message);
        Web3Error::Transport(message)
    })?;
    // Log the raw text before decoding to get more information on responses that aren't valid
    // json. Debug encoding so we don't get control characters like newlines in the output.
    tracing::debug!("[id:{}] received response: {:?}", id, text.trim());
    if !status.is_success() {
        return Err(Web3Error::Transport(format!(
            "response status code is not success: {}",
            status
        )));
    }
    jsonrpc_core::serde_from_str(&text).map_err(Into::into)
}

type RpcResult = Result<Value, Web3Error>;

impl Transport for HttpTransport {
    type Out = BoxFuture<'static, RpcResult>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        let id = self.next_id();
        let request = helpers::build_request(id, method, params);
        (id, request)
    }

    fn send(&self, id: RequestId, call: Call) -> Self::Out {
        let (client, url) = self.new_request();
        async move {
            let _guard = TransportMetrics::instance().on_request_start(method_name(&call));

            let output = execute_rpc(client, url, id, &Request::Single(call)).await?;
            helpers::to_result_from_output(output)
        }
        .boxed()
    }
}

impl BatchTransport for HttpTransport {
    type Batch = BoxFuture<'static, Result<Vec<RpcResult>, Web3Error>>;

    fn send_batch<T>(&self, requests: T) -> Self::Batch
    where
        T: IntoIterator<Item = (RequestId, Call)>,
    {
        // Batch calls don't need an id but it helps associate the response log to the request log.
        let id = self.next_id();
        let (client, url) = self.new_request();
        let (ids, calls): (Vec<_>, Vec<_>) = requests.into_iter().unzip();
        async move {
            let _guard = TransportMetrics::instance().on_request_start("batch");

            let outputs = execute_rpc(client, url, id, &Request::Batch(calls)).await?;
            handle_batch_response(&ids, outputs)
        }
        .boxed()
    }
}

/// Workaround for Erigon nodes, which encode each element of the Batch Response as a String rather than a deserializable JSON object
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum OutputOrString {
    String(String),
    Output(Output),
}

impl OutputOrString {
    fn try_into_output(self) -> Result<Output, Web3Error> {
        Ok(match self {
            OutputOrString::String(string) => jsonrpc_core::serde_from_str(&string)?,
            OutputOrString::Output(output) => output,
        })
    }
}

fn handle_batch_response(
    ids: &[RequestId],
    outputs: Vec<OutputOrString>,
) -> Result<Vec<RpcResult>, Web3Error> {
    if ids.len() != outputs.len() {
        return Err(Web3Error::InvalidResponse(
            "unexpected number of responses".to_string(),
        ));
    }
    let mut outputs = outputs
        .into_iter()
        .map(|output_or_string| {
            let output = output_or_string.try_into_output()?;
            Ok((
                id_of_output(&output)?,
                helpers::to_result_from_output(output),
            ))
        })
        .collect::<Result<HashMap<_, _>, Web3Error>>()?;
    ids.iter()
        .map(|id| {
            outputs.remove(id).ok_or_else(|| {
                Web3Error::InvalidResponse(format!("batch response is missing id {}", id))
            })
        })
        .collect()
}

fn id_of_output(output: &Output) -> Result<RequestId, Web3Error> {
    let id = match output {
        Output::Success(success) => &success.id,
        Output::Failure(failure) => &failure.id,
    };
    match id {
        jsonrpc_core::Id::Num(num) => Ok(*num as RequestId),
        _ => Err(Web3Error::InvalidResponse(
            "response id is not u64".to_string(),
        )),
    }
}

fn method_name(call: &Call) -> &str {
    match call {
        Call::MethodCall(method) => &method.method,
        Call::Notification(notification) => &notification.method,
        Call::Invalid { .. } => "invalid",
    }
}

struct TransportMetrics {
    requests_inflight: prometheus::IntGaugeVec,
    requests_complete: prometheus::CounterVec,
    requests_duration_seconds: prometheus::HistogramVec,
}

impl TransportMetrics {
    fn new(registry: &Registry) -> prometheus::Result<Self> {
        let label_names = &["method"];

        let opts = prometheus::Opts::new(
            "node_transport_requests_inflight",
            "Number of inflight RPC requests for ethereum node",
        );
        let requests_inflight = prometheus::IntGaugeVec::new(opts, label_names)?;
        registry.register(Box::new(requests_inflight.clone()))?;

        let opts = prometheus::Opts::new(
            "node_transport_requests_complete",
            "Number of completed RPC requests for ethereum node",
        );
        let requests_complete = prometheus::CounterVec::new(opts, label_names)?;
        registry.register(Box::new(requests_complete.clone()))?;

        let opts = prometheus::HistogramOpts::new(
            "node_transport_requests_duration_seconds",
            "Execution time for each RPC request (batches are counted as one request)",
        );
        let requests_duration_seconds = prometheus::HistogramVec::new(opts, label_names)?;
        registry.register(Box::new(requests_duration_seconds.clone()))?;

        Ok(TransportMetrics {
            requests_inflight,
            requests_complete,
            requests_duration_seconds,
        })
    }

    fn instance() -> &'static Self {
        lazy_static::lazy_static! {
            static ref INSTANCE: TransportMetrics =
                TransportMetrics::new(get_metrics_registry()).unwrap();
        }

        &INSTANCE
    }

    #[must_use]
    fn on_request_start(&self, method: &str) -> impl Drop {
        let requests_inflight = self.requests_inflight.with_label_values(&[method]);
        let requests_complete = self.requests_complete.with_label_values(&[method]);
        let requests_duration_seconds = self.requests_duration_seconds.with_label_values(&[method]);

        requests_inflight.inc();
        let timer = requests_duration_seconds.start_timer();

        scopeguard::guard(timer, move |timer| {
            requests_inflight.dec();
            requests_complete.inc();
            timer.stop_and_record();
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_batch_response_being_in_different_order_than_input() {
        let ids = vec![0, 1, 2];
        // This order is different from the ids.
        let outputs = [1u64, 0, 2]
            .iter()
            .map(|&id| {
                OutputOrString::Output(Output::Success(jsonrpc_core::Success {
                    jsonrpc: None,
                    result: id.into(),
                    id: jsonrpc_core::Id::Num(id),
                }))
            })
            .collect();
        let results = handle_batch_response(&ids, outputs)
            .unwrap()
            .into_iter()
            .map(|result| result.unwrap().as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        // The order of the ids should have been restored.
        assert_eq!(ids, results);
    }

    #[test]
    fn handles_batch_items_that_are_strings() {
        let result = handle_batch_response(
            &[1],
            vec![OutputOrString::String("{\"result\": 1, \"id\": 1}".into())],
        )
        .unwrap()
        .into_iter()
        .map(|result| result.unwrap().as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
        assert_eq!(vec![1], result);
    }

    #[test]
    fn errors_on_invalid_string_batch_responses() {
        assert!(handle_batch_response(
            &[1],
            vec![OutputOrString::String("there is no spoon".into())],
        )
        .is_err());
    }
}
