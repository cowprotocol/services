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
use url::Url;
pub use {
    anyhow::{Error, Result},
    contracts,
    model::order::OrderData,
    shared::web3::Web3,
};

/// Builds a web3 client that sends requests one by one.
pub fn unbuffered_web3(ethrpc: &Url) -> Web3 {
    shared::web3::web3(
        &shared::web3::Arguments {
            ethrpc_max_batch_size: 0,
            ethrpc_max_concurrent_requests: 0,
            ethrpc_batch_delay: Default::default(),
        },
        ethrpc,
        "base",
    )
}
