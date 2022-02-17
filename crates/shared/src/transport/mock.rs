//! Mockable Web3 transport implementation.

use ethcontract::{
    futures::future::{self, Ready},
    jsonrpc::{Call, Id, MethodCall, Params},
    web3::{self, BatchTransport, RequestId, Transport},
    Web3,
};
use serde_json::Value;
use std::{
    fmt::{self, Debug, Formatter},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

pub fn web3() -> Web3<MockTransport> {
    Web3::new(MockTransport::new())
}

/// An intermediate trait used for `mockall` to automatically generate a mock
/// transport for us.
#[mockall::automock]
pub trait MockableTransport {
    fn execute(&self, method: String, params: Vec<Value>) -> web3::Result<Value>;
    fn execute_batch(
        &self,
        requests: Vec<(String, Vec<Value>)>,
    ) -> web3::Result<Vec<web3::Result<Value>>>;
}

#[derive(Clone, Default)]
pub struct MockTransport(Arc<Inner>);

#[derive(Default)]
pub struct Inner {
    inner: Mutex<MockMockableTransport>,
    current_id: AtomicUsize,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mock(&self) -> MutexGuard<MockMockableTransport> {
        self.0.inner.lock().unwrap()
    }
}

impl Debug for MockTransport {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("MockTransport").finish()
    }
}

impl Transport for MockTransport {
    type Out = Ready<web3::Result<Value>>;

    fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
        let id = self.0.current_id.fetch_add(1, Ordering::SeqCst);
        (
            id,
            Call::MethodCall(MethodCall {
                jsonrpc: None,
                method: method.to_owned(),
                params: Params::Array(params),
                id: Id::Null,
            }),
        )
    }

    fn send(&self, _: RequestId, call: Call) -> Self::Out {
        let (method, params) = extract_call(call);
        let response = self.mock().execute(method, params);
        future::ready(response)
    }
}

impl BatchTransport for MockTransport {
    type Batch = Ready<web3::Result<Vec<web3::Result<Value>>>>;

    fn send_batch<T>(&self, requests: T) -> Self::Batch
    where
        T: IntoIterator<Item = (RequestId, Call)>,
    {
        let batch = requests
            .into_iter()
            .map(|(_, call)| extract_call(call))
            .collect();
        let responses = self.mock().execute_batch(batch);
        future::ready(responses)
    }
}

fn extract_call(call: Call) -> (String, Vec<Value>) {
    match call {
        Call::MethodCall(MethodCall {
            method,
            params: Params::Array(params),
            ..
        }) => (method, params),
        _ => panic!("unexpected call {:?}", call),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use serde_json::json;

    #[tokio::test]
    async fn can_mock_single_requests() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute()
            .with(
                eq("foo_bar".to_owned()),
                eq(vec![json!(true), json!("stuff")]),
            )
            .returning(|_, _| Ok(json!("hello")));

        assert_eq!(
            transport
                .execute("foo_bar", vec![json!(true), json!("stuff")])
                .await
                .unwrap(),
            json!("hello")
        );
    }

    #[tokio::test]
    async fn can_mock_batch_requests() {
        let transport = MockTransport::new();
        transport
            .mock()
            .expect_execute_batch()
            .with(eq(vec![
                ("foo_bar".to_owned(), vec![json!(true), json!("stuff")]),
                ("do_thing".to_owned(), vec![]),
                ("fail_thing".to_owned(), vec![json!(42)]),
            ]))
            .returning(|_| {
                Ok(vec![
                    Ok(json!("hello")),
                    Ok(json!("world")),
                    Err(web3::Error::Transport(
                        web3::error::TransportError::Message("bad".to_string()),
                    )),
                ])
            });

        let responses = transport
            .send_batch(vec![
                transport.prepare("foo_bar", vec![json!(true), json!("stuff")]),
                transport.prepare("do_thing", vec![]),
                transport.prepare("fail_thing", vec![json!(42)]),
            ])
            .await
            .unwrap();
        assert_eq!(responses[0].as_ref().unwrap(), &json!("hello"));
        assert_eq!(responses[1].as_ref().unwrap(), &json!("world"));
        assert!(responses[2].is_err());
    }
}
