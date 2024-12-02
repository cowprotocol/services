//! This is a very simple anti-corruption layer between the driver and the rest
//!     of the codebase. The purpose of this layer is to give a very clear
//!     indication of where and how the integration between the driver and the
//!     rest of the code happens, and to serve as a line of defense against
//!     leaking unnecessary details from that codebase into the driver.
//!
//! To quote a popular author:
//!
//! > When a new system is being built that must have a large interface with
//! > another, the difficulty of relating the two models can eventually
//! > overwhelm
//! > the intent of the new model altogether, causing it to be modified to
//! > resemble the other system's model, in an ad hoc fashion. The models of
//! > legacy systems are usually weak, and even the exception that is well
//! > developed may not fit the needs of the current project. Yet there may be a
//! > lot of value in the integration, and sometimes it is an absolute
//! > requirement. Therefore, the developer should create an isolating layer to
//! > provide clients with functionality in terms of their own domain model. The
//! > layer talks to the other system through its existing interface, requiring
//! > little or no modification to the other system. Internally, the layer
//! > translates in both directions as necessary between the two models.
//!
//! By Eric Evans, Domain-Driven Design: Tackling Complexity in the Heart of
//! Software (2014)

pub mod liquidity;

// The [`anyhow::Error`] type is re-exported because the legacy code mostly
// returns that error. This will change as the legacy code gets refactored away.
use {crate::infra::blockchain::Ethereum, url::Url};
pub use {
    anyhow::{Error, Result},
    contracts,
    model::order::OrderData,
    shared::ethrpc::Web3,
};

/// Returns a Web3 instance with a trait object transport needed by various
/// boundary components.
fn web3(eth: &Ethereum) -> Web3 {
    // Ugly way to get access to one of these... However, this way we don't
    // leak this into our domain logic.
    eth.contracts().settlement().raw_instance().web3()
}

/// Builds a web3 client that buffers requests and sends them in a
/// batch call.
pub fn buffered_web3_client(ethrpc: &Url) -> Web3 {
    web3_client(ethrpc, 20, 10)
}

/// Builds a web3 client that sends requests one by one.
pub fn unbuffered_web3_client(ethrpc: &Url) -> Web3 {
    web3_client(ethrpc, 0, 0)
}

fn web3_client(ethrpc: &Url, max_batch_size: usize, max_concurrent_requests: usize) -> Web3 {
    let ethrpc_args = shared::ethrpc::Arguments {
        ethrpc_max_batch_size: max_batch_size,
        ethrpc_max_concurrent_requests: max_concurrent_requests,
        ethrpc_batch_delay: Default::default(),
    };
    let http_factory =
        shared::http_client::HttpClientFactory::new(&shared::http_client::Arguments {
            http_timeout: std::time::Duration::from_secs(10),
        });
    shared::ethrpc::web3(&ethrpc_args, &http_factory, ethrpc, "base")
}
