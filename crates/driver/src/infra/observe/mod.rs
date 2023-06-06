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
                solution::{self, settlement, Settlement},
                Auction,
                Reveal,
                Solution,
            },
            quote::{self, Quote},
            Liquidity,
        },
        util::http,
    },
    url::Url,
};

mod metrics;

// TODO The idea is that some of the functions below will also update the
// metrics. This should be fairly easy to do, just go down the list and think
// about if the thing is worth measuring or not. This will be done in a
// follow-up PR.

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
    tracing::info!(?liquidity, "fetched liquidity");
}

/// Observe that fetching liquidity failed.
pub fn fetching_liquidity_failed(err: &boundary::Error) {
    tracing::warn!(?err, "failed to fetch liquidity");
}

/// Observe the solutions returned by the solver.
pub fn solutions(solutions: &[Solution]) {
    tracing::info!(?solutions, "solutions");
}

/// Observe that a solution was discarded because it is empty.
pub fn empty_solution(id: solution::Id) {
    tracing::info!(?id, "discarded solution: empty");
}

/// Observe that a solution is about to be encoded into a settlement.
pub fn encoding(id: solution::Id) {
    tracing::trace!(?id, "encoding settlement");
}

/// Observe that settlement encoding failed.
pub fn encoding_failed(id: solution::Id, err: &solution::Error) {
    tracing::info!(?id, ?err, "discarded solution: settlement encoding failed");
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
        settlement_id = ?settlement.id,
        "scoring settlement"
    );
}

/// Observe that scoring failed.
pub fn scoring_failed(id: settlement::Id, err: &boundary::Error) {
    tracing::info!(?id, ?err, "discarded solution: scoring failed");
}

/// Observe the settlement score.
pub fn score(settlement: &Settlement, score: &solution::Score) {
    tracing::info!(
        solutions = ?settlement.solutions(),
        settlement_id = ?settlement.id,
        score = score.0.to_f64_lossy(),
        "settlement scored"
    );
}

/// Observe that the settlement process is about to start.
pub fn settling(id: settlement::Id) {
    tracing::trace!(?id, "settling");
}

/// Observe the result of the settlement process.
pub fn settled(id: settlement::Id, result: &Result<(), competition::Error>) {
    match result {
        Ok(()) => tracing::info!(?id, "settled"),
        Err(err) => tracing::warn!(?id, ?err, "failed to settle"),
    }
}

/// Observe the result of solving an auction.
pub fn solved(auction: &Auction, result: &Result<Reveal, competition::Error>) {
    match result {
        Ok(reveal) => tracing::info!(?auction, ?reveal, "solved auction"),
        Err(err) => tracing::warn!(?auction, ?err, "failed to solve auction"),
    }
}

/// Observe the result of quoting an auction.
pub fn quoted(order: &quote::Order, result: &Result<Quote, quote::Error>) {
    match result {
        Ok(quote) => tracing::info!(?order, ?quote, "quoted order"),
        Err(err) => tracing::warn!(?order, ?err, "failed to quote order"),
    }
}

/// Observe that the API routes for a solver are being mounted.
pub fn mounting_solver(path: &str) {
    tracing::debug!(path, "mounting solver");
}

/// Observe that a request is about to be sent to the solver.
pub fn solver_request(endpoint: &Url, req: &str) {
    tracing::trace!(%endpoint, %req, "sending request to solver");
}

/// Observe that a response was received from the solver.
pub fn solver_response(endpoint: &Url, res: Result<&str, &http::Error>) {
    match res {
        Ok(res) => tracing::trace!(%endpoint, %res, "received response from solver"),
        Err(err) => tracing::warn!(%endpoint, ?err, "failed to receive response from solver"),
    }
}

/// Observe that a mempool failed to send a transaction.
pub fn mempool_failed(mempool: &Mempool, err: &boundary::Error) {
    tracing::warn!(?err, ?mempool, "sending transaction via mempool failed");
}

/// Observe that an invalid DTO was received.
pub fn invalid_dto(err: &impl std::error::Error, endpoint: &str, what: &str) {
    tracing::warn!(?err, "invalid {what} dto received in {endpoint}");
}

/// Observe that the quoting process is about to start.
pub fn quoting(order: &quote::Order) {
    tracing::trace!(?order, "quoting");
}
