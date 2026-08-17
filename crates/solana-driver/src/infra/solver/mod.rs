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
    futures::future::join_all,
    solana_sdk::pubkey::Pubkey,
    std::sync::Arc,
    thiserror::Error,
    tokio::sync::Semaphore,
    tracing::Instrument,
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
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap();
        Self {
            name: config.name.clone(),
            account: config.account,
            client,
            base_url: config.endpoint.clone(),
            in_flight: Arc::new(Semaphore::new(config.max_in_flight.get())),
        }
    }

    /// The solver's human-readable name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The solver's on-chain identity.
    pub fn account(&self) -> Pubkey {
        self.account
    }

    /// POST the auction to this engine's `/solve` endpoint and return the
    /// domain solutions it produced.
    #[tracing::instrument(name = "solver_engine", skip_all, fields(solver = %self.name))]
    pub async fn solve(&self, auction: &domain::Auction) -> Result<Vec<domain::Solution>, Error> {
        let auction_dto = Auction::new(auction, self.account);
        let body = serde_json::to_string(&auction_dto)?;

        let solve_url = self.base_url.join("solve").expect("valid /solve path");
        let _permit = self.in_flight.acquire().await;
        let request = self
            .client
            .post(solve_url.as_str())
            .header("content-type", "application/json")
            .body(body);

        tracing::debug!(url = %solve_url, "sending solve request");

        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Error::HttpStatus { status, body });
        }

        let solutions: dto::Solutions = serde_json::from_str(&body)?;
        solutions
            .into_domain(&auction_dto, self.account)
            .map_err(Error::BadResponse)
    }
}

/// Query every configured solver engine for solutions to the given auction.
///
/// An engine that fails, exceeds the timeout, or returns bad data loses this
/// auction. The other engines still compete.
pub async fn solve_all(solvers: &[Solver], auction: &domain::Auction) -> Vec<domain::Solution> {
    if solvers.is_empty() {
        return Vec::new();
    }

    let futures = solvers.iter().map(|solver| {
        let solver = solver.clone();
        let auction = auction.clone();
        let name = solver.name().to_string();
        async move {
            let result = solver.solve(&auction).await;
            match result {
                Ok(solutions) => {
                    tracing::info!(
                        solver = %solver.name(),
                        count = solutions.len(),
                        "solver returned solutions"
                    );
                    solutions
                }
                Err(err) => {
                    tracing::warn!(solver = %solver.name(), ?err, "solver failed");
                    Vec::new()
                }
            }
        }
        .instrument(tracing::info_span!("solve", solver = name))
    });

    join_all(futures).await.into_iter().flatten().collect()
}

#[derive(Debug, Error)]
pub enum Error {
    /// The solver did not respond within the configured timeout.
    #[error("solver timed out")]
    Timeout,
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
}
