//! Helpers for mocking the JSON-RPC node the solver uses for simulations.

use {super::http, serde_json::json};

/// Expectation for the `eth_call` simulating a DEX swap. Responds with the
/// given amount of gas used by the swap.
pub fn gas_simulation(gas: u64) -> http::Expectation {
    http::Expectation::Post {
        path: http::Path::Any,
        req: http::RequestBody::Any,
        res: json!({
            "id": 0,
            "jsonrpc": "2.0",
            "result": format!("0x{gas:064x}"),
        }),
    }
}
