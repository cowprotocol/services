//! This module implements the observability for the driver. It exposes
//! functions which represent events that are meaningful to the system. These
//! functions are called when the corresponding events occur. They log the event
//! and update the metrics, if the event is worth measuring.

use {
    super::Mempool,
    crate::{
        boundary,
        domain::{
            competition::{
                self,
                auction,
                solution::{self, Settlement},
                Auction,
                Solution,
                Solved,
            },
            eth,
            quote::{self, Quote},
            Liquidity,
        },
        infra::solver,
        util::http,
    },
    url::Url,
};

mod metrics;

/// Setup the observability. The log argument configures the tokio tracing
/// framework.
pub fn init(log: &str) {
    boundary::initialize_tracing(log);
    metrics::init();
}

/// Observe a received auction.
pub fn auction(solver: &solver::Name, auction: &Auction) {
    tracing::info!(%solver, ?auction, "received auction");
}

/// Observe that liquidity fetching is about to start.
pub fn fetching_liquidity() {
    tracing::trace!("fetching liquidity");
}

/// Observe the fetched liquidity.
pub fn fetched_liquidity(liquidity: &[Liquidity]) {
    tracing::info!(?liquidity, "fetched liquidity");
}

/// Observe that fetching liquidity failed.
pub fn fetching_liquidity_failed(err: &boundary::Error) {
    tracing::warn!(?err, "failed to fetch liquidity");
}

/// Observe the solutions returned by the solver.
pub fn solutions(solver: &solver::Name, solutions: &[Solution]) {
    tracing::info!(%solver, ?solutions, "solutions");
}

/// Observe that a solution was discarded because it is empty.
pub fn empty_solution(solver: &solver::Name, id: solution::Id) {
    tracing::info!(%solver, ?id, "discarded solution: empty");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "EmptySolution"])
        .inc();
}

/// Observe that a solution is about to be encoded into a settlement.
pub fn encoding(solver: &solver::Name, id: solution::Id) {
    tracing::trace!(%solver, ?id, "encoding settlement");
}

/// Observe that settlement encoding failed.
pub fn encoding_failed(solver: &solver::Name, id: solution::Id, err: &solution::Error) {
    tracing::info!(%solver, ?id, ?err, "discarded solution: settlement encoding failed");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "SettlementEncodingFailed"])
        .inc();
}

/// Observe that two solutions were merged.
pub fn merged(solver: &solver::Name, settlement: &Settlement, other: &Settlement) {
    tracing::debug!(
        %solver,
        settlement_1 = ?settlement.solutions(),
        settlement_2 = ?other.solutions(),
        "merged solutions"
    );
}

/// Observe that it was not possible to merge two solutions.
pub fn not_merged(
    solver: &solver::Name,
    settlement: &Settlement,
    other: &Settlement,
    err: solution::Error,
) {
    tracing::debug!(
        %solver,
        ?err,
        settlement_1 = ?settlement.solutions(),
        settlement_2 = ?other.solutions(),
        "solutions can't be merged"
    );
}

/// Observe that scoring is about to start.
pub fn scoring(solver: &solver::Name, settlement: &Settlement) {
    tracing::trace!(
        %solver,
        solutions = ?settlement.solutions(),
        auction_id = ?settlement.auction_id,
        "scoring settlement"
    );
}

/// Observe that scoring failed.
pub fn scoring_failed(solver: &solver::Name, auction_id: auction::Id, err: &boundary::Error) {
    tracing::info!(%solver, ?auction_id, ?err, "discarded solution: scoring failed");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "ScoringFailed"])
        .inc();
}

/// Observe the settlement score.
pub fn score(solver: &solver::Name, settlement: &Settlement, score: &solution::Score) {
    tracing::info!(
        %solver,
        solutions = ?settlement.solutions(),
        auction_id = ?settlement.auction_id,
        score = score.0.to_f64_lossy(),
        "settlement scored"
    );
}

pub fn revealing(solver: &solver::Name, auction_id: Option<auction::Id>) {
    tracing::trace!(%solver, ?auction_id, "revealing");
}

pub fn revealed(
    solver: &solver::Name,
    auction_id: Option<auction::Id>,
    result: &Result<competition::Revealed, competition::Error>,
) {
    match result {
        Ok(calldata) => {
            tracing::info!(%solver, ?calldata, ?auction_id, "revealed");
            metrics::get()
                .reveals
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(%solver, ?auction_id, ?err, "failed to reveal");
            metrics::get()
                .reveals
                .with_label_values(&[solver.as_str(), competition_error(err)])
                .inc();
        }
    }
}

/// Observe that the settlement process is about to start.
pub fn settling(solver: &solver::Name, auction_id: Option<auction::Id>) {
    tracing::trace!(%solver, ?auction_id, "settling");
}

/// Observe the result of the settlement process.
pub fn settled(
    solver: &solver::Name,
    auction_id: Option<auction::Id>,
    result: &Result<competition::Settled, competition::Error>,
) {
    match result {
        Ok(calldata) => {
            tracing::info!(%solver, ?calldata, ?auction_id, "settled");
            metrics::get()
                .settlements
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(%solver, ?err, ?auction_id, "failed to settle");
            metrics::get()
                .settlements
                .with_label_values(&[solver.as_str(), competition_error(err)])
                .inc();
        }
    }
}

/// Observe the result of solving an auction.
pub fn solved(
    solver: &solver::Name,
    auction: &Auction,
    result: &Result<Solved, competition::Error>,
) {
    match result {
        Ok(reveal) => {
            tracing::info!(%solver, ?auction, ?reveal, "solved auction");
            metrics::get()
                .solutions
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(%solver, ?auction, ?err, "failed to solve auction");
            metrics::get()
                .solutions
                .with_label_values(&[solver.as_str(), competition_error(err)])
                .inc();
        }
    }
}

/// Observe the result of quoting an auction.
pub fn quoted(solver: &solver::Name, order: &quote::Order, result: &Result<Quote, quote::Error>) {
    match result {
        Ok(quote) => {
            tracing::info!(%solver, ?order, ?quote, "quoted order");
            metrics::get()
                .quotes
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(%solver, ?order, ?err, "failed to quote order");
            metrics::get()
                .quotes
                .with_label_values(&[
                    solver.as_str(),
                    match err {
                        quote::Error::QuotingFailed(quote::QuotingFailed::ClearingSellMissing) => {
                            "ClearingSellMissing"
                        }
                        quote::Error::QuotingFailed(quote::QuotingFailed::ClearingBuyMissing) => {
                            "ClearingBuyMissing"
                        }
                        quote::Error::QuotingFailed(quote::QuotingFailed::NoSolutions) => {
                            "NoSolutions"
                        }
                        quote::Error::DeadlineExceeded(_) => "DeadlineExceeded",
                        quote::Error::Blockchain(_) => "BlockchainError",
                        quote::Error::Solver(solver::Error::Http(_)) => "SolverHttpError",
                        quote::Error::Solver(solver::Error::Deserialize(_)) => {
                            "SolverDeserializeError"
                        }
                        quote::Error::Solver(solver::Error::RepeatedSolutionIds) => {
                            "RepeatedSolutionIds"
                        }
                        quote::Error::Solver(solver::Error::Dto(_)) => "SolverDtoError",
                        quote::Error::Boundary(_) => "Unknown",
                    },
                ])
                .inc();
        }
    }
}

/// Observe that the API routes for a solver are being mounted.
pub fn mounting_solver(solver: &solver::Name, path: &str) {
    tracing::debug!(%solver, path, "mounting solver");
}

/// Observe that a request is about to be sent to the solver.
pub fn solver_request(solver: &solver::Name, endpoint: &Url, req: &str) {
    tracing::trace!(%solver, %endpoint, %req, "sending request to solver");
}

/// Observe that a response was received from the solver.
pub fn solver_response(solver: &solver::Name, endpoint: &Url, res: Result<&str, &http::Error>) {
    match res {
        Ok(res) => {
            tracing::trace!(%solver, %endpoint, %res, "received response from solver")
        }
        Err(err) => {
            tracing::warn!(%solver, %endpoint, ?err, "failed to receive response from solver")
        }
    }
}

/// Observe the result of mempool transaction execution.
pub fn mempool_executed(
    solver: &solver::Name,
    mempool: &Mempool,
    settlement: &Settlement,
    res: &Result<eth::TxId, boundary::Error>,
) {
    match res {
        Ok(txid) => {
            tracing::info!(
                %solver, ?txid, ?mempool, ?settlement,
                "sending transaction via mempool succeeded",
            );
        }
        Err(err) => {
            tracing::warn!(
                %solver, ?err, ?mempool, ?settlement,
                "sending transaction via mempool failed",
            );
        }
    }
}

/// Observe that an invalid DTO was received.
pub fn invalid_dto(
    solver: &solver::Name,
    err: &impl std::error::Error,
    endpoint: &str,
    what: &str,
) {
    tracing::warn!(%solver, ?err, "invalid {what} dto received in {endpoint}");
}

/// Observe that the quoting process is about to start.
pub fn quoting(solver: &solver::Name, order: &quote::Order) {
    tracing::trace!(%solver, ?order, "quoting");
}

fn competition_error(err: &competition::Error) -> &'static str {
    match err {
        competition::Error::SolutionNotAvailable => "SolutionNotAvailable",
        competition::Error::SolutionNotFound => "SolutionNotFound",
        competition::Error::DeadlineExceeded(_) => "DeadlineExceeded",
        competition::Error::Solver(solver::Error::Http(_)) => "SolverHttpError",
        competition::Error::Solver(solver::Error::Deserialize(_)) => "SolverDeserializeError",
        competition::Error::Solver(solver::Error::RepeatedSolutionIds) => "RepeatedSolutionIds",
        competition::Error::Solver(solver::Error::Dto(_)) => "SolverDtoError",
    }
}
