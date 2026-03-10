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
sell-orders-endpoint = 'http://{solver_addr}/'
api-project-id = '1'
api-key = '1234'
api-secret-key = '1234567890123456'
api-passphrase = 'pass'
",
    ))
}
