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
pub mod order_creation_simulation;
pub mod order_quoting;
pub mod order_validation;
pub mod remaining_amounts;
pub mod retry;
pub mod token_list;
pub mod url;
pub mod web3;

use {
    alloy::primitives::Address,
    std::{collections::HashSet, sync::LazyLock},
};

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}

pub fn ban_list() -> &'static HashSet<Address> {
    static BAN_LIST: LazyLock<HashSet<Address>> = LazyLock::new(|| {
        let Ok(raw) = std::env::var("BAN_LIST") else {
            return HashSet::new();
        };
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse()
                    .unwrap_or_else(|e| panic!("invalid address {s:?} in BAN_LIST: {e}"))
            })
            .collect()
    });
    &BAN_LIST
}
