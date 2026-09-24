//! HTTP client for solver engines.
//!
//! The driver posts each auction to the configured engines on `/solve` and
//! collects their solutions. Engines are opaque HTTP services.
//! Solver-specific behavior lives in the solver engine crate, not here.

use {
    crate::{
        domain::{self, solver_fee::SolverFee},
        infra::{config, signer, solver::dto::auction::Auction},
    },
    solana_sdk::pubkey::Pubkey,
    std::{num::NonZero, sync::Arc},
    thiserror::Error,
};

pub mod dto;

/// A configured solver engine HTTP client.
#[derive(Clone)]
pub struct Solver {
    name: String,
    signer: Arc<signer::Signer>,
    client: reqwest::Client,
    base_url: reqwest::Url,
    solve_every_nth_auction: Option<NonZero<u64>>,
    solver_fee: Option<SolverFee>,
}

impl Solver {
    /// The human-readable name of this solver, for logs and metrics.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The solver's on-chain identity, derived from its signer.
    pub fn pubkey(&self) -> Pubkey {
        self.signer.pubkey()
    }

    /// The solver's settlement signer.
    pub(crate) fn signer(&self) -> &signer::Signer {
        &self.signer
    }

    /// The auction-id stride this solver participates at, when throttled.
    pub fn solve_every_nth_auction(&self) -> Option<NonZero<u64>> {
        self.solve_every_nth_auction
    }

    /// The volume-based solver fee, `None` when no fee is configured.
    pub fn solver_fee(&self) -> Option<SolverFee> {
        self.solver_fee
    }

    /// Build a solver client from its configuration, loading the signer the
    /// config names: a local keypair file or an AWS KMS key.
    pub async fn new(config: &config::Solver) -> Result<Self, Error> {
        let signer = match &config.signer {
            config::SettlementSigner::Keypair(path) => {
                let keypair =
                    solana_sdk::signer::keypair::read_keypair_file(path).map_err(|error| {
                        Error::SignerKeypair {
                            solver: config.name.clone(),
                            path: path.clone(),
                            error: error.to_string().into(),
                        }
                    })?;
                signer::Signer::Keypair(Arc::new(keypair))
            }
            config::SettlementSigner::KmsKey(key_id) => {
                signer::Signer::Kms(signer::KmsSigner::new(key_id.clone()).await.map_err(
                    |error| Error::Signer {
                        solver: config.name.clone(),
                        error,
                    },
                )?)
            }
        };
        tracing::info!(
            solver = %config.name,
            pubkey = %signer.pubkey(),
            "loaded solver signer"
        );
        Ok(Self {
            name: config.name.clone(),
            signer: Arc::new(signer),
            client: reqwest::Client::new(),
            base_url: config.endpoint.clone(),
            solve_every_nth_auction: config.solve_every_nth_auction,
            solver_fee: config.solver_fee_bps,
        })
    }

    /// POST the auction to this engine's `/solve` endpoint and return the
    /// domain solutions it produced.
    ///
    /// `program_id` is the settlement program the swap instructions are built
    /// for.
    #[tracing::instrument(name = "solver_engine", skip_all, fields(solver = %self.name))]
    pub async fn solve(
        &self,
        auction: &domain::Auction,
        program_id: Pubkey,
    ) -> Result<Vec<domain::Solution>, Error> {
        let auction_dto = Auction::new(auction, self.pubkey(), program_id, self.solver_fee);
        let body = serde_json::to_string(&auction_dto)?;

        let solve_url = self.base_url.join("solve").expect("valid /solve path");

        // Calculate the time remaining until the auction's deadline.
        //
        // TODO: Split the deadline budget between solver time and driver
        // processing time. Give the solver a configurable fraction of the
        // remaining time and reserve the rest for building the transaction.
        let timeout = {
            let remaining = auction.deadline.signed_duration_since(chrono::Utc::now());
            if remaining <= chrono::Duration::zero() {
                tracing::warn!(
                    solver = %self.name,
                    "auction deadline exceeded before sending request to solver"
                );
                return Ok(Default::default());
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
            .into_domain(&auction_dto, self.pubkey())
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
    /// The signer keypair could not be loaded from the configured path.
    #[error("failed to load signer keypair for solver {solver} from {path}: {error}")]
    SignerKeypair {
        solver: String,
        path: std::path::PathBuf,
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The KMS signer could not be set up.
    #[error("failed to load the KMS signer for solver {solver}: {error}")]
    Signer {
        solver: String,
        #[source]
        error: signer::Error,
    },
}

#[cfg(test)]
mod tests {
    use {super::*, solana_testlib::temp_keypair};

    #[tokio::test]
    async fn solve_with_past_deadline_returns_empty() {
        // Build a solver pointing at a port that is never listened on. The
        // deadline check fires before any HTTP request is sent, so this never
        // actually connects to the endpoint.
        let keypair_file = temp_keypair();
        let keypair_path = keypair_file.path().to_path_buf();
        let solver = Solver::new(&config::Solver {
            name: "test".to_owned(),
            endpoint: "http://127.0.0.1:1".parse().unwrap(),
            signer: config::SettlementSigner::Keypair(keypair_path),
            solve_every_nth_auction: None,
            solver_fee_bps: None,
        })
        .await
        .expect("solver construction should succeed");
        let auction = domain::Auction {
            id: Some(domain::Id::new(1).unwrap()),
            orders: Vec::new(),
            deadline_slot: domain::Slot(1),
            // Well in the past: the request must be skipped entirely.
            deadline: chrono::Utc::now() - chrono::Duration::seconds(10),
        };

        let solutions = solver
            .solve(&auction, Pubkey::default())
            .await
            .expect("solve should succeed with no solutions");
        assert!(
            solutions.is_empty(),
            "expected empty solutions, got {solutions:?}"
        );
    }
}
