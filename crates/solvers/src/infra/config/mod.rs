use std::fmt::Debug;

pub mod baseline;
pub mod dex;
pub mod legacy;
pub mod naive;

/// Parameters used to calculate the revert risk of a solution.
#[derive(Debug, Clone)]
pub struct RiskParameters {
    pub gas_amount_factor: f64,
    pub gas_price_factor: f64,
    pub nmb_orders_factor: f64,
    pub intercept: f64,
}

/// Unwraps result or logs a `TOML` parsing error.
fn unwrap_or_log<T, E, P>(result: Result<T, E>, path: &P) -> T
where
    E: Debug,
    P: Debug,
{
    result.unwrap_or_else(|err| {
        if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
            panic!("failed to parse TOML config at {path:?}: {err:#?}")
        } else {
            panic!(
                "failed to parse TOML config at: {path:?}. Set TOML_TRACE_ERROR=1 to print \
                 parsing error but this may leak secrets."
            )
        }
    })
}
