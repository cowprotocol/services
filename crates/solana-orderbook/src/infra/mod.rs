//! Infrastructure layer: concrete implementations of the orderbook's external
//! dependencies (configuration, database, HTTP API, observability).

pub mod api;
pub mod config;
pub mod observe;

pub use self::api::Api;
