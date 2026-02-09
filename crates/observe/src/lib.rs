//! This crate is intended to contain code that is required to provide or
//! improve the observability of a system. That includes initialization logic
//! for metrics and logging as well as logging helper functions.
pub mod config;
pub mod distributed_tracing;
pub mod future;
#[cfg(unix)]
pub mod heap_dump_handler;
pub mod metrics;
pub mod panic_hook;
pub mod tracing;
#[cfg(unix)]
mod tracing_reload_handler;

pub use {
    config::{Config, TracingConfig},
    distributed_tracing::request_id,
};
