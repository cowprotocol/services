//! HTTP client for solver engines.
//!
//! The driver posts each auction to the configured engines on `/solve` and
//! collects their solutions. Engines are opaque HTTP services.
//! Solver-specific behavior lives in the solver engine crate, not here.

use {
    crate::{
        domain::{
            self,
            order_uid::OrderUid,
            settlement::MaxNativeShortfall,
            solver_fee::SolverFee,
        },
        infra::{config, solver::dto::auction::Auction},
    },
    cow_solana_signer::Signer,
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::{collections::HashSet, num::NonZero, sync::Arc, time::Duration},
    thiserror::Error,
};

pub mod dto;

/// A configured solver engine HTTP client.
#[derive(Clone)]
pub struct Solver {
    name: String,
    signer: Arc<Signer>,
    client: reqwest::Client,
    base_url: reqwest::Url,
    solve_every_nth_auction: Option<NonZero<u64>>,
    solver_fee: Option<SolverFee>,
    max_native_shortfall: Option<MaxNativeShortfall>,
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
    pub(crate) fn signer(&self) -> &Signer {
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

    /// The largest share of the native SOL payouts the solver's own SOL
    /// covers, `None` when it covers none.
    pub fn max_native_shortfall(&self) -> Option<MaxNativeShortfall> {
        self.max_native_shortfall
    }

    /// Build a solver client from its configuration, loading the signer the
    /// config names: a local keypair file or an AWS KMS key.
    pub async fn new(config: &config::Solver) -> Result<Self, Error> {
        let signer = config.signer.load().await.map_err(|error| Error::Signer {
            solver: config.name.clone(),
            error,
        })?;
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
            max_native_shortfall: config.max_native_shortfall_bps,
        })
    }

    /// POST the auction to this engine's `/solve` endpoint and return the
    /// domain solutions it produced.
    ///
    /// `program_id` is the settlement program the swap instructions are built
    /// for. `missing_buy_token_accounts` marks the orders whose payout account
    /// the settlement creates.
    #[tracing::instrument(name = "solver_engine", skip_all, fields(solver = %self.name))]
    pub async fn solve(
        &self,
        auction: &domain::Auction,
        program_id: Pubkey,
        missing_buy_token_accounts: &HashSet<OrderUid>,
    ) -> Result<Vec<domain::Solution>, Error> {
        let auction_dto = Auction::new(
            auction,
            self.pubkey(),
            program_id,
            self.solver_fee,
            missing_buy_token_accounts,
        );
        let body = serde_json::to_string(&auction_dto)?;

        let solve_url = self.base_url.join("solve").expect("valid /solve path");

        // Calculate the time remaining until the auction's deadline.
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

    /// POST the signed `transaction` to this engine's `/cosign` endpoint for
    /// the signatures of its other required signers.
    #[tracing::instrument(name = "solver_engine", skip_all, fields(solver = %self.name))]
    pub async fn cosign(
        &self,
        transaction: &VersionedTransaction,
        timeout: Duration,
    ) -> Result<VersionedTransaction, Error> {
        let body = serde_json::to_string(&dto::cosign::Transaction::new(transaction))?;
        let url = self.base_url.join("cosign").expect("valid /cosign path");
        let response = self
            .client
            .post(url.as_str())
            .header("content-type", "application/json")
            .timeout(timeout)
            .body(body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus {
                status,
                body: response.text().await?,
            });
        }
        let cosigned = response
            .json::<dto::cosign::Transaction>()
            .await?
            .into_transaction()
            .map_err(Error::Cosign)?;
        verify_cosigned(transaction, &cosigned)?;
        Ok(cosigned)
    }
}

/// The co-signed transaction must be `own` with the empty slots filled: the
/// same message, the same driver signature, and every signature valid.
fn verify_cosigned(
    own: &VersionedTransaction,
    cosigned: &VersionedTransaction,
) -> Result<(), Error> {
    if cosigned.message != own.message {
        return Err(Error::Cosign("the message changed".into()));
    }
    if cosigned.signatures.len() != own.signatures.len()
        || cosigned.signatures.first() != own.signatures.first()
    {
        return Err(Error::Cosign(
            "the signature slots or the driver's signature changed".into(),
        ));
    }
    if cosigned.verify_with_results().contains(&false) {
        return Err(Error::Cosign("a signature does not verify".into()));
    }
    Ok(())
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
    /// The configured signer could not be loaded.
    #[error("failed to load the signer for solver {solver}: {error}")]
    Signer {
        solver: String,
        #[source]
        error: cow_solana_signer::Error,
    },
    /// The engine's co-signed transaction is unusable.
    #[error("co-signing failed: {0}")]
    Cosign(String),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::{
            hash::Hash,
            message::{VersionedMessage, v0},
            signature::Signature,
            signer::{Signer as _, keypair::Keypair},
        },
        solana_testlib::temp_keypair,
    };

    /// A transaction the driver signed and `cosigner` still has to, and the
    /// same transaction with the co-signature filled in.
    fn signed_and_cosigned(
        driver: &Keypair,
        cosigner: &Keypair,
    ) -> (VersionedTransaction, VersionedTransaction) {
        let instruction = solana_system_interface::instruction::transfer(
            &cosigner.pubkey(),
            &Pubkey::new_unique(),
            1,
        );
        let message = VersionedMessage::V0(
            v0::Message::try_compile(&driver.pubkey(), &[instruction], &[], Hash::new_unique())
                .unwrap(),
        );
        let bytes = message.serialize();
        let own = VersionedTransaction {
            signatures: vec![driver.sign_message(&bytes), Signature::default()],
            message,
        };
        let mut cosigned = own.clone();
        cosigned.signatures[1] = cosigner.sign_message(&bytes);
        (own, cosigned)
    }

    #[test]
    fn rejects_a_changed_message() {
        let (driver, cosigner) = (Keypair::new(), Keypair::new());
        let (own, _) = signed_and_cosigned(&driver, &cosigner);
        let (_, other) = signed_and_cosigned(&driver, &cosigner);
        assert!(verify_cosigned(&own, &other).is_err());
    }

    #[test]
    fn rejects_a_replaced_driver_signature() {
        let (own, mut cosigned) = signed_and_cosigned(&Keypair::new(), &Keypair::new());
        cosigned.signatures[0] = Signature::default();
        assert!(verify_cosigned(&own, &cosigned).is_err());
    }

    /// Verification pairs signatures with keys up to the shorter list, so a
    /// dropped slot would otherwise verify.
    #[test]
    fn rejects_a_dropped_signature() {
        let (own, mut cosigned) = signed_and_cosigned(&Keypair::new(), &Keypair::new());
        cosigned.signatures.pop();
        assert!(verify_cosigned(&own, &cosigned).is_err());
    }

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
            signer: cow_solana_signer::Config::Keypair(keypair_path),
            solve_every_nth_auction: None,
            solver_fee_bps: None,
            max_native_shortfall_bps: None,
        })
        .await
        .expect("solver construction should succeed");
        let auction = domain::Auction {
            id: Some(domain::Id::new(1).unwrap()),
            orders: Vec::new(),
            deadline_slot: domain::Slot(1),
            // Well in the past: the request must be skipped entirely.
            deadline: chrono::Utc::now() - chrono::Duration::seconds(10),
            creations: std::collections::HashMap::new(),
            native_prices: std::collections::HashMap::new(),
        };

        let solutions = solver
            .solve(&auction, Pubkey::default(), &Default::default())
            .await
            .expect("solve should succeed with no solutions");
        assert!(
            solutions.is_empty(),
            "expected empty solutions, got {solutions:?}"
        );
    }
}
