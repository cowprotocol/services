use {crate::tests, std::net::SocketAddr};

mod api_calls;
mod market_order;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
[dex]
chain-id = '1'
endpoint = 'http://{solver_addr}/bgw-pro/swapx/pro/'
[dex.credentials]
api-key = 'test-api-key'
api-secret = 'test-api-secret-1234'
",
    ))
}
