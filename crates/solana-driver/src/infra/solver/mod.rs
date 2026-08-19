//! HTTP client for solver engines.
//!
//! The driver posts each auction to the configured engines on `/solve` and
//! collects their solutions. Engines are opaque HTTP services. All
//! Jupiter-specific behavior lives in the `solana-solvers` crate, not here.

use {
    crate::{
        domain,
        infra::{config, solver::dto::auction::Auction},
    },
    solana_sdk::pubkey::Pubkey,
    std::sync::Arc,
    thiserror::Error,
    tokio::sync::Semaphore,
};

pub mod dto;

/// A configured solver engine HTTP client.
#[derive(Debug, Clone)]
pub struct Solver {
    name: String,
    account: Pubkey,
    client: reqwest::Client,
    base_url: reqwest::Url,
    in_flight: Arc<Semaphore>,
}

impl Solver {
    /// Build a solver client from its configuration.
    pub fn new(config: &config::Solver) -> Self {
        Self {
            name: config.name.clone(),
            account: config.account,
            client: reqwest::Client::new(),
            base_url: config.endpoint.clone(),
            in_flight: Arc::new(Semaphore::new(config.max_in_flight.get())),
        }
    }

    /// POST the auction to this engine's `/solve` endpoint and return the
    /// domain solutions it produced.
    #[tracing::instrument(name = "solver_engine", skip_all, fields(solver = %self.name))]
    pub async fn solve(&self, auction: &domain::Auction) -> Result<Vec<domain::Solution>, Error> {
        let auction_dto = Auction::new(auction, self.account);
        let body = serde_json::to_string(&auction_dto)?;

        let solve_url = self.base_url.join("solve").expect("valid /solve path");

        let _permit = self
            .in_flight
            .acquire()
            .await
            .expect("semaphore is never closed");

        // Calculate the time remaining until the auction's deadline. This is
        // computed *after* acquiring the permit, otherwise the wait could
        // silently eat into the budget and let the solve run past the deadline.
        //
        // TODO: Split the deadline budget between solver time and driver processing
        // time. The EVM driver uses `solving_share_of_deadline` to give the solver a
        // configurable fraction of the remaining time, leaving the rest for building
        let timeout = {
            let remaining = auction.deadline.signed_duration_since(chrono::Utc::now());
            if remaining <= chrono::Duration::zero() {
                tracing::warn!(
                    solver = %self.name,
                    "auction deadline exceeded before sending request to solver"
                );
                return Err(Error::DeadlineExceeded);
            }
            // Safe: we just checked `remaining` is positive.
            remaining.to_std().unwrap()
        };
        let request = self
            .client
            .post(solve_url.as_str())
            .header("content-type", "application/json")
            .timeout(timeout)
            .body(body);

        tracing::debug!(url = %solve_url, "sending solve request");

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus {
                status,
                body: response.text().await?,
            });
        }

        let solutions: dto::Solutions = response.json().await?;
        solutions
            .into_domain(&auction_dto, self.account)
            .map_err(Error::BadResponse)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    /// An HTTP error occurred while talking to the solver.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// The solver returned a non-success HTTP status.
    #[error("solver returned HTTP {status}: {body}")]
    HttpStatus {
        status: reqwest::StatusCode,
        body: String,
    },
    /// The solver returned a response the driver could not interpret.
    #[error("bad solver response: {0}")]
    BadResponse(#[from] dto::solution::Error),
    /// The request body could not be serialized.
    #[error("JSON serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    /// The auction deadline passed before a solve request could be sent.
    #[error("auction deadline exceeded")]
    DeadlineExceeded,
}

#[cfg(test)]
mod tests {
    use {super::*, std::num::NonZero};

    #[tokio::test]
    async fn solve_with_past_deadline_is_rejected() {
        // Build a solver pointing at a port that is never listened on. The
        // deadline check fires before any HTTP request is sent, so this never
        // actually connects to the endpoint.
        let solver = Solver::new(&config::Solver {
            name: "test".to_owned(),
            endpoint: "http://127.0.0.1:1".parse().unwrap(),
            account: Pubkey::default(),
            max_in_flight: NonZero::new(1).unwrap(),
        });
        let auction = domain::Auction {
            id: 0,
            orders: Vec::new(),
            // Well in the past: the request must be skipped entirely.
            deadline: chrono::Utc::now() - chrono::Duration::seconds(10),
        };

        let err = solver.solve(&auction).await.expect_err("solve should fail");
        assert!(
            matches!(err, Error::DeadlineExceeded),
            "expected DeadlineExceeded, got {err:?}"
        );
    }
}
