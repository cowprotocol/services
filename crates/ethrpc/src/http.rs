use {
    ethcontract::jsonrpc as jsonrpc_core,
    futures::{future::BoxFuture, FutureExt},
    jsonrpc_core::types::{Call, Output, Request, Value},
    reqwest::{header, Client, Url},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{Debug, Formatter},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    },
    web3::{
        error::{Error as Web3Error, TransportError},
        helpers,
        BatchTransport,
        RequestId,
        Transport,
    },
};

#[derive(Clone)]
pub struct HttpTransport {
    client: Client,
    inner: Arc<Inner>,
}

struct Inner {
    url: Url,
    id: AtomicUsize,
    /// Name of the transport used in logs to distinguish different transports.
    name: String,
}

impl HttpTransport {
    pub fn new(client: Client, url: Url, name: String) -> Self {
        Self {
            client,
            inner: Arc::new(Inner {
                url,
                id: AtomicUsize::new(0),
                name,
            }),
        }
    }

    fn next_id(&self) -> RequestId {
        self.inner.id.fetch_add(1, Ordering::SeqCst)
    }

    fn new_request(&self) -> (Client, Arc<Inner>) {
        (self.client.clone(), self.inner.clone())
    }
}

impl Debug for HttpTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport")
            .field("url", &self.inner.url)
            .finish()
    }
}

// Id is only used for logging.
async fn execute_rpc<T: DeserializeOwned>(
    client: Client,
    inner: Arc<Inner>,
    id: RequestId,
    request: &Request,
) -> Result<T, Web3Error> {
    let body = serde_json::to_string(&request)?;
    tracing::trace!(name = %inner.name, %id, %body, "executing request");
    let mut request_builder = client
        .post(inner.url.clone())
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-RPC-REQUEST-ID", id.to_string())
        .body(body);
    if let Some(request_id) = observe::request_id::get_task_local_storage() {
        request_builder = request_builder.header("X-REQUEST-ID", request_id);
    }
    match request {
        Request::Single(Call::MethodCall(method)) => {
            request_builder = request_builder.header("X-RPC-METHOD", method.method.clone());
        }
        Request::Batch(calls) => {
            let methods = calls
                .iter()
                .filter_map(|call| match call {
                    Call::MethodCall(method) => Some(method.method.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(",");
            request_builder = request_builder.header("X-RPC-METHOD", methods);
        }
        _ => {}
    }

    let response = request_builder
        .send()
        .await
        .map_err(|err: reqwest::Error| {
            tracing::warn!(name = %inner.name, %id, %err, "failed to send request");
            Web3Error::Transport(TransportError::Message(err.to_string()))
        })?;
    let status = response.status();
    let text = response.text().await.map_err(|err: reqwest::Error| {
        tracing::warn!(name = %inner.name, %id, %err, "failed to get response body");
        Web3Error::Transport(TransportError::Message(err.to_string()))
    })?;
    // Log the raw text before decoding to get more information on responses that
    // aren't valid json. Debug encoding so we don't get control characters like
    // newlines in the output.
    tracing::trace!(name = %inner.name, %id, body = %text.trim(), "received response");
    if !status.is_success() {
        return Err(Web3Error::Transport(TransportError::Message(format!(
            "HTTP error {status}"
        ))));
    }

    let result = jsonrpc_core::serde_from_str(&text).map_err(|err| {
        Web3Error::Decoder(format!(
            "{:?}, raw response: {}, {}, {}",
            err,
            inner.name,
            id,
            text.trim()
        ))
    })?;
    Ok(result)
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
        let (client, inner) = self.new_request();

        async move {
            let output = execute_rpc(client, inner, id, &Request::Single(call)).await?;
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
        // Batch calls don't need an id but it helps associate the response log to the
        // request log.
        let id = self.next_id();
        let (client, inner) = self.new_request();
        let (ids, calls): (Vec<_>, Vec<_>) = requests.into_iter().unzip();

        async move {
            let outputs = execute_rpc(client, inner, id, &Request::Batch(calls)).await?;
            handle_batch_response(&ids, outputs)
        }
        .boxed()
    }
}

/// Workaround for Erigon nodes, which encode each element of the Batch Response
/// as a String rather than a deserializable JSON object
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
                Web3Error::InvalidResponse(format!("batch response is missing id {id}"))
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
