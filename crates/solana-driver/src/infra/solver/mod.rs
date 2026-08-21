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
    solana_sdk::{
        pubkey::Pubkey,
        signer::{Signer, keypair::Keypair},
    },
    std::sync::Arc,
    thiserror::Error,
    tokio::sync::Semaphore,
};

pub mod dto;

/// A configured solver engine HTTP client.
#[derive(Clone)]
pub struct Solver {
    name: String,
    account: Pubkey,
    #[expect(dead_code, reason = "used by the settlement path in follow-up PRs")]
    signer: Arc<Keypair>,
    client: reqwest::Client,
    base_url: reqwest::Url,
    in_flight: Arc<Semaphore>,
}

impl Solver {
    /// The human-readable name of this solver, for logs and metrics.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Build a solver client from its configuration.
    ///
    /// Loads the signer keypair from `config.signer_keypair` and validates that
    /// its public key matches `config.account`.
    pub fn new(config: &config::Solver) -> Result<Self, Error> {
        let keypair = solana_sdk::signer::keypair::read_keypair_file(&config.signer_keypair)
            .map_err(|error| Error::SignerKeypair {
                solver: config.name.clone(),
                path: config.signer_keypair.clone(),
                error,
            })?;
        let pubkey = keypair.pubkey();
        if pubkey != config.account {
            return Err(Error::SignerPubkeyMismatch {
                solver: config.name.clone(),
                pubkey,
                account: config.account,
            });
        }
        Ok(Self {
            name: config.name.clone(),
            account: config.account,
            signer: Arc::new(keypair),
            client: reqwest::Client::new(),
            base_url: config.endpoint.clone(),
            in_flight: Arc::new(Semaphore::new(config.max_in_flight.get())),
        })
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
    /// The signer keypair could not be loaded from the configured path.
    #[error("failed to load signer keypair for solver {solver} from {path}: {error}")]
    SignerKeypair {
        solver: String,
        path: std::path::PathBuf,
        #[source]
        error: Box<dyn std::error::Error>,
    },
    /// The signer keypair's public key does not match the solver's configured
    /// on-chain identity.
    #[error(
        "signer keypair pubkey {pubkey} does not match solver account {account} for solver \
         {solver}"
    )]
    SignerPubkeyMismatch {
        solver: String,
        pubkey: Pubkey,
        account: Pubkey,
    },
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::signer::keypair::{Keypair, read_keypair_file, write_keypair_file},
        std::num::NonZero,
        tempfile::NamedTempFile,
    };

    /// Write a fresh keypair to a temp file and return its path.
    fn temp_keypair() -> std::path::PathBuf {
        let file = NamedTempFile::new().expect("create temp file");
        let path = file.into_temp_path().keep().expect("keep temp file");
        write_keypair_file(&Keypair::new(), &path).expect("write keypair");
        path
    }

    #[tokio::test]
    async fn solve_with_past_deadline_is_rejected() {
        // Build a solver pointing at a port that is never listened on. The
        // deadline check fires before any HTTP request is sent, so this never
        // actually connects to the endpoint.
        let keypair_path = temp_keypair();
        let solver = Solver::new(&config::Solver {
            name: "test".to_owned(),
            endpoint: "http://127.0.0.1:1".parse().unwrap(),
            account: read_keypair_file(&keypair_path).unwrap().pubkey(),
            signer_keypair: keypair_path,
            max_in_flight: NonZero::new(1).unwrap(),
        })
        .expect("solver construction should succeed");
        let auction = domain::Auction {
            id: domain::Id::new(1).unwrap(),
            orders: Vec::new(),
            deadline_slot: domain::Slot(1),
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
