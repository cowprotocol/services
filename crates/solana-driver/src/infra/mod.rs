//! Infrastructure layer: concrete implementations of the driver's external
//! dependencies (configuration, RPC, HTTP API, observability).

pub mod api;
pub mod config;
pub mod observe;

pub use self::api::Api;
