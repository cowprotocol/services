use {crate::tests, std::net::SocketAddr};

mod attaching_approvals;
mod jit_order;
mod market_order;

/// Creates a legacy solver configuration for the specified host.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
solver-name = 'legacy_solver'
endpoint = 'http://{solver_addr}/solve'
chain-id = '1'
        ",
    ))
}
