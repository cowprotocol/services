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
    std::collections::HashMap,
    url::Url,
};

mod metrics;

/// Setup the observability. The log argument configures the tokio tracing
/// framework.
pub fn init(log: &str) {
    observe::tracing::initialize_reentrant(log);
    metrics::init();
}

/// Observe a received auction.
pub fn auction(auction: &Auction) {
    tracing::info!(?auction, "received auction");
}

/// Observe that liquidity fetching is about to start.
pub fn fetching_liquidity() {
    tracing::trace!("fetching liquidity");
}

/// Observe the fetched liquidity.
pub fn fetched_liquidity(liquidity: &[Liquidity]) {
    let mut grouped: HashMap<&'static str, usize> = Default::default();
    for liquidity in liquidity {
        *grouped.entry((&liquidity.kind).into()).or_default() += 1;
    }
    tracing::info!(liquidity = ?grouped, "fetched liquidity sources");
}

/// Observe that fetching liquidity failed.
pub fn fetching_liquidity_failed(err: &boundary::Error) {
    tracing::warn!(?err, "failed to fetch liquidity");
}

/// Observe the solutions returned by the solver.
pub fn solutions(solutions: &[Solution]) {
    tracing::info!(?solutions, "computed solutions");
}

/// Observe that a solution was discarded because it is empty.
pub fn empty_solution(solver: &solver::Name, id: solution::Id) {
    tracing::info!(?id, "discarded solution: empty");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "EmptySolution"])
        .inc();
}

/// Observe that a solution is about to be encoded into a settlement.
pub fn encoding(id: solution::Id) {
    tracing::trace!(?id, "encoding settlement");
}

/// Observe that settlement encoding failed.
pub fn encoding_failed(solver: &solver::Name, id: solution::Id, err: &solution::Error) {
    tracing::info!(?id, ?err, "discarded solution: settlement encoding failed");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "SettlementEncodingFailed"])
        .inc();
}

/// Observe that two solutions were merged.
pub fn merged(settlement: &Settlement, other: &Settlement) {
    tracing::debug!(
        settlement_1 = ?settlement.solutions(),
        settlement_2 = ?other.solutions(),
        "merged solutions"
    );
}

/// Observe that it was not possible to merge two solutions.
pub fn not_merged(settlement: &Settlement, other: &Settlement, err: solution::Error) {
    tracing::debug!(
        ?err,
        settlement_1 = ?settlement.solutions(),
        settlement_2 = ?other.solutions(),
        "solutions can't be merged"
    );
}

/// Observe that scoring is about to start.
pub fn scoring(settlement: &Settlement) {
    tracing::trace!(
        solutions = ?settlement.solutions(),
        "scoring settlement"
    );
}

/// Observe that scoring failed.
pub fn scoring_failed(solver: &solver::Name, err: &boundary::Error) {
    tracing::info!(%solver, ?err, "discarded solution: scoring failed");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "ScoringFailed"])
        .inc();
}

/// Observe the settlement score.
pub fn score(settlement: &Settlement, score: &solution::Score) {
    tracing::info!(
        solutions = ?settlement.solutions(),
        score = score.0.to_f64_lossy(),
        "scored settlement"
    );
}

pub fn revealing() {
    tracing::trace!("revealing");
}

pub fn revealed(solver: &solver::Name, result: &Result<competition::Revealed, competition::Error>) {
    match result {
        Ok(calldata) => {
            tracing::info!(?calldata, "revealed");
            metrics::get()
                .reveals
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(?err, "failed to reveal");
            metrics::get()
                .reveals
                .with_label_values(&[solver.as_str(), competition_error(err)])
                .inc();
        }
    }
}

/// Observe that the settlement process is about to start.
pub fn settling() {
    tracing::trace!("settling solution");
}

/// Observe the result of the settlement process.
pub fn settled(solver: &solver::Name, result: &Result<competition::Settled, competition::Error>) {
    match result {
        Ok(calldata) => {
            tracing::info!(?calldata, "settled solution");
            metrics::get()
                .settlements
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(?err, "failed to settle");
            metrics::get()
                .settlements
                .with_label_values(&[solver.as_str(), competition_error(err)])
                .inc();
        }
    }
}

/// Observe the result of solving an auction.
pub fn solved(solver: &solver::Name, result: &Result<Solved, competition::Error>) {
    match result {
        Ok(solved) => {
            tracing::info!(?solved, "solved auction");
            metrics::get()
                .solutions
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(?err, "failed to solve auction");
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
            tracing::info!(?order, ?quote, "quoted order");
            metrics::get()
                .quotes
                .with_label_values(&[solver.as_str(), "Success"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(?order, ?err, "failed to quote order");
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
pub fn solver_request(endpoint: &Url, req: &str) {
    tracing::trace!(%endpoint, %req, "sending request to solver");
}

/// Observe that a response was received from the solver.
pub fn solver_response(endpoint: &Url, res: Result<&str, &http::Error>) {
    match res {
        Ok(res) => {
            tracing::trace!(%endpoint, %res, "received response from solver")
        }
        Err(err) => {
            tracing::warn!(%endpoint, ?err, "failed to receive response from solver")
        }
    }
}

/// Observe the result of mempool transaction execution.
pub fn mempool_executed(
    mempool: &Mempool,
    settlement: &Settlement,
    res: &Result<eth::TxId, boundary::Error>,
) {
    match res {
        Ok(txid) => {
            tracing::info!(
                ?txid,
                ?mempool,
                ?settlement,
                "sending transaction via mempool succeeded",
            );
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                ?mempool,
                ?settlement,
                "sending transaction via mempool failed",
            );
        }
    }
}

/// Observe that an invalid DTO was received.
pub fn invalid_dto(err: &impl std::error::Error, dto: &str) {
    tracing::warn!(?err, ?dto, "received invalid dto");
}

/// Observe that the quoting process is about to start.
pub fn quoting(order: &quote::Order) {
    tracing::trace!(?order, "quoting");
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

#[derive(Debug)]
pub enum OrderExcludedFromAuctionReason<'a> {
    CouldNotFetchBalance(&'a crate::infra::blockchain::Error),
    CouldNotCalculateMaxSell,
    InsufficientBalance,
    OrderWithZeroAmountRemaining,
}

pub fn order_excluded_from_auction(
    order: &competition::Order,
    reason: OrderExcludedFromAuctionReason,
) {
    tracing::trace!(uid=?order.uid, ?reason, "order excluded from auction");
}
