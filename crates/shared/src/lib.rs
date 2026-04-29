#[macro_use]
pub mod macros;

pub mod arguments;
pub mod current_block;
pub mod db_order_conversions;
pub mod event_storing_helpers;
pub mod external_prices;
pub mod fee;
pub mod http_solver;
pub mod interaction;
pub mod order_quoting;
pub mod order_validation;
pub mod remaining_amounts;
pub mod retry;
pub mod token_list;
pub mod url;
pub mod web3;

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}
