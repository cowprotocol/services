use {crate::tests, std::net::SocketAddr};

mod api_calls;
mod market_order;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the default config (buy orders
/// disabled).
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

/// Creates a config with buy orders enabled via the reverse-quote endpoint.
pub fn config_with_buy_orders(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
[dex]
chain-id = '1'
endpoint = 'http://{solver_addr}/bgw-pro/swapx/pro/'
enable-buy-orders = true
[dex.credentials]
api-key = 'test-api-key'
api-secret = 'test-api-secret-1234'
",
    ))
}
