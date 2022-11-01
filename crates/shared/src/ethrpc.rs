pub mod buffered;
pub mod dummy;
pub mod extensions;
pub mod http;
pub mod mock;

use self::{buffered::BufferedTransport, http::HttpTransport};
use crate::http_client::HttpClientFactory;
use ethcontract::{batch::CallBatch, dyns::DynWeb3, transport::DynTransport};
use reqwest::{Client, Url};
use std::convert::TryInto as _;

pub const MAX_BATCH_SIZE: usize = 100;

pub type Web3 = DynWeb3;
pub type Web3Transport = DynTransport;
pub type Web3CallBatch = CallBatch<Web3Transport>;

/// Create a Web3 instance.
pub fn web3(http_factory: &HttpClientFactory, url: &Url, name: impl ToString) -> Web3 {
    let transport = Web3Transport::new(BufferedTransport::new(HttpTransport::new(
        http_factory.configure(|builder| builder.cookie_store(true)),
        url.clone(),
        name.to_string(),
    )));
    Web3::new(transport)
}

/// Convenience method to create a transport from a URL.
pub fn create_test_transport(url: &str) -> Web3Transport {
    Web3Transport::new(HttpTransport::new(
        Client::new(),
        url.try_into().unwrap(),
        "".to_string(),
    ))
}

/// Like above but takes url from the environment NODE_URL.
pub fn create_env_test_transport() -> Web3Transport {
    create_test_transport(&std::env::var("NODE_URL").unwrap())
}
