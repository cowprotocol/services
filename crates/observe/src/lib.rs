//! This crate is intended to contain code that is required to provide or
//! improve the observability of a system. That includes initialization logic
//! for metrics and logging as well as logging helper functions.
pub mod config;
pub mod future;
pub mod metrics;
pub mod panic_hook;
pub mod request_id;
pub mod trace_id_format;
pub mod tracing;
#[cfg(feature = "axum-tracing")]
pub mod tracing_axum;
#[cfg(unix)]
mod tracing_reload_handler;
pub mod tracing_warp;

pub use config::{Config, TracingConfig};
