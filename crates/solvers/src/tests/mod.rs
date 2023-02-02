//! Solver engine end-to-end tests.
//!
//! Note that this is setup as a "unit test" in that it is part of the `src/`
//! directory. This is done intentionally as Cargo builds separate binaries for
//! each file in `tests/`, which makes `cargo test` slower.

mod baseline;

use {
    reqwest::Url,
    tokio::{sync::oneshot, task::JoinHandle},
};

/// A solver engine handle for E2E testing.
pub struct SolverEngine {
    url: Url,
    handle: JoinHandle<()>,
}

impl SolverEngine {
    /// Creates a new solver engine handle for the specified command
    /// configuration.
    pub async fn new(command: String, config: String) -> Self {
        let (bind, bind_receiver) = oneshot::channel();

        let handle = tokio::spawn(crate::run::run(
            vec![
                "/test/solvers/path".to_owned(),
                "--chain-id".to_owned(),
                "1".to_owned(),
                "--addr".to_owned(),
                "0.0.0.0:0".to_owned(),
                "--config".to_owned(),
                config,
                command,
            ]
            .into_iter(),
            Some(bind),
        ));

        let addr = bind_receiver.await.unwrap();
        let url = format!("http://{addr}/").parse().unwrap();

        Self { url, handle }
    }

    /// Solves a raw JSON auction.
    pub async fn solve(&self, auction: serde_json::Value) -> serde_json::Value {
        let client = reqwest::Client::new();
        let response = client
            .post(self.url.clone())
            .json(&auction)
            .send()
            .await
            .unwrap();

        if !response.status().is_success() {
            panic!(
                "HTTP {}: {:?}",
                response.status(),
                response.text().await.unwrap(),
            );
        }

        response.json().await.unwrap()
    }
}

impl Drop for SolverEngine {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
