use std::fmt::Debug;

pub mod baseline;
pub mod legacy;
pub mod naive;

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
