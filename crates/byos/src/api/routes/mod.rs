mod proposals;
mod solve;

pub use {
    proposals::{cancel_proposal, get_proposals, submit_proposal},
    solve::solve,
};

pub async fn healthz() -> &'static str {
    "OK"
}

pub async fn metrics() -> String {
    let registry = observe::metrics::get_registry();
    observe::metrics::encode(registry)
}
