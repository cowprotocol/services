//! This is a abstraction layer between the solver engine and the existing
//! "legacy" logic in `shared` and `model`.

pub mod baseline;
pub mod liquidity;

pub use shared::{exit_process_on_panic, tracing::initialize_reentrant as initialize_tracing};
