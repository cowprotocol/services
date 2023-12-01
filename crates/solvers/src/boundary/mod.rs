//! This is a abstraction layer between the solver engine and the existing
//! "legacy" logic in `shared` and `model`.

pub mod baseline;
pub mod legacy;
pub mod liquidity;
pub mod naive;
pub mod rate_limiter;

pub type Result<T> = anyhow::Result<T>;
