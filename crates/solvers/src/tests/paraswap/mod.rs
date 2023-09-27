use {crate::tests, std::net::SocketAddr};

mod market_order;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
[dex]
endpoint = 'http://{solver_addr}'
exclude-dexs = ['UniswapV2']
address = '0xE0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1'
partner = 'cow'
        ",
    ))
}
