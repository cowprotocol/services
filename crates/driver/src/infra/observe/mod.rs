//! This module implements the observability for the driver. It exposes
//! functions which represent events that are meaningful to the system. These
//! functions are called when the corresponding events occur. They log the event
//! and update the metrics, if the event is worth measuring.

use {
    super::{Ethereum, Mempool, simulator, solver::Timeouts},
    crate::{
        boundary,
        domain::{
            Liquidity,
            competition::{
                self,
                Solution,
                Solved,
                solution::{self, Settlement},
            },
            eth::{self, Gas},
            mempools::{self, SubmissionSuccess},
            quote::{self, Quote},
            time::{Deadline, Remaining},
        },
        infra::solver,
        util::http,
    },
    ethrpc::block_stream::BlockInfo,
    std::{
        collections::{BTreeMap, HashSet},
        time::Duration,
    },
    url::Url,
};

pub mod metrics;

/// Setup the observability. The log argument configures the tokio tracing
/// framework.
pub fn init(obs_config: observe::Config) {
    observe::tracing::initialize_reentrant(&obs_config);
    metrics::init();
    #[cfg(unix)]
    observe::heap_dump_handler::spawn_heap_dump_handler();
}

/// Observe a received auction.
pub fn auction(auction_id: i64) {
    tracing::debug!(id=?auction_id, "received auction");
}

/// Observe that liquidity fetching is about to start.
pub fn fetching_liquidity() {
    tracing::trace!("fetching liquidity");
}

/// Observe the fetched liquidity.
pub fn fetched_liquidity(liquidity: &[Liquidity]) {
    let mut grouped: BTreeMap<&'static str, usize> = Default::default();
    for liquidity in liquidity {
        *grouped.entry((&liquidity.kind).into()).or_default() += 1;
    }
    tracing::debug!(liquidity = ?grouped, "fetched liquidity sources");
}

/// Observe that fetching liquidity failed.
pub fn fetching_liquidity_failed(err: &boundary::Error) {
    tracing::warn!(?err, "failed to fetch liquidity");
}

pub fn duplicated_solution_id(solver: &solver::Name, id: &solution::Id) {
    tracing::debug!(?id, "discarded solution: duplicated id");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "DuplicateId"])
        .inc();
}

/// Observe the solutions returned by the solver.
pub fn solutions(
    solutions: &[Solution],
    surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
) {
    if solutions
        .iter()
        .any(|s| !s.is_empty(surplus_capturing_jit_order_owners))
    {
        tracing::info!(?solutions, "computed solutions");
    } else {
        tracing::debug!("no solutions");
    }
}

/// Observe that a solution was discarded because it is empty.
pub fn empty_solution(solver: &solver::Name, id: &solution::Id) {
    tracing::debug!(?id, "discarded solution: empty");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "EmptySolution"])
        .inc();
}

// Observe that postprocessing (encoding & merging) of solutions is about to
// start.
pub fn postprocessing(solutions: &[Solution], deadline: chrono::DateTime<chrono::Utc>) {
    tracing::debug!(
        solutions = ?solutions.len(),
        remaining = ?deadline.remaining(),
        "postprocessing solutions"
    );
}

// Observe that postprocessing didn't complete before the timeout.
pub fn postprocessing_timed_out(completed: &[Settlement]) {
    tracing::debug!(
        completed = ?completed.len(),
        "postprocessing solutions timed out"
    );
}

/// Observe that a solution is about to be encoded into a settlement.
pub fn encoding(id: &solution::Id) {
    tracing::trace!(?id, "encoding settlement");
}

/// Observe that settlement encoding failed.
pub fn encoding_failed(
    solver: &solver::Name,
    id: &solution::Id,
    err: &solution::Error,
    has_haircut: bool,
) {
    tracing::info!(
        ?id,
        ?err,
        has_haircut,
        "discarded solution: settlement encoding"
    );
    let reason = if has_haircut {
        "SettlementEncodingHaircut"
    } else {
        "SettlementEncoding"
    };
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), reason])
        .inc();
}

/// Observe that two solutions were merged.
pub fn merged(first: &Solution, other: &Solution, result: &Solution) {
    tracing::trace!(?first, ?other, ?result, "merged solutions");
}

/// Observe that scoring is about to start.
pub fn scoring(settlement: &Settlement) {
    tracing::trace!(
        solution = ?settlement.solution(),
        gas = ?settlement.gas,
        "scoring settlement"
    );
}

/// Observe that scoring failed.
pub fn scoring_failed(solver: &solver::Name, err: &solution::error::Scoring) {
    tracing::info!(%solver, ?err, "discarded solution: scoring");
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), "Scoring"])
        .inc();
}

/// Observe the settlement score.
pub fn score(settlement: &Settlement, score: &eth::Ether) {
    tracing::info!(
        solution = ?settlement.solution(),
        score = ?score,
        "scored settlement"
    );
}

// Observe that the winning settlement started failing upon arrival of a new
// block
pub fn winner_voided(
    solver: &solver::Name,
    block: BlockInfo,
    err: &simulator::RevertError,
    has_haircut: bool,
) {
    tracing::warn!(
        block = block.number,
        ?err,
        has_haircut,
        "solution reverts on new block"
    );
    let reason = if has_haircut {
        "SimulationRevertHaircut"
    } else {
        "SimulationRevert"
    };
    metrics::get()
        .dropped_solutions
        .with_label_values(&[solver.as_str(), reason])
        .inc();
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
pub fn solved(solver: &str, result: &Result<Option<Solved>, competition::Error>) {
    match result {
        Ok(Some(solved)) => {
            tracing::info!(?solved, "solved auction");
            metrics::get()
                .solutions
                .with_label_values(&[solver, "Success"])
                .inc();
        }
        Ok(None) => {
            tracing::debug!("no solution found");
            metrics::get()
                .solutions
                .with_label_values(&[solver, "SolutionNotFound"])
                .inc();
        }
        Err(err) => {
            tracing::warn!(?err, "failed to solve auction");
            metrics::get()
                .solutions
                .with_label_values(&[solver, competition_error(err)])
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
                        quote::Error::QuotingFailed(quote::QuotingFailed::Math) => "MathError",
                        quote::Error::DeadlineExceeded(_) => "DeadlineExceeded",
                        quote::Error::Blockchain(_) => "BlockchainError",
                        quote::Error::Solver(solver::Error::Http(_)) => "SolverHttpError",
                        quote::Error::Solver(solver::Error::Deserialize(_)) => {
                            "SolverDeserializeError"
                        }
                        quote::Error::Solver(solver::Error::Dto(_)) => "SolverDtoError",
                        quote::Error::Boundary(_) => "Unknown",
                        quote::Error::Encoding(_) => "Encoding",
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
pub fn solver_response(
    endpoint: &Url,
    res: Result<&str, &http::Error>,
    solver: &str,
    compute_time: Duration,
    is_quote_request: bool,
) {
    match res {
        Ok(res) => {
            tracing::trace!(%endpoint, %res, "received response from solver")
        }
        Err(err) => {
            tracing::warn!(%endpoint, ?err, "failed to receive response from solver")
        }
    }
    let kind = if is_quote_request { "quote" } else { "auction" };
    metrics::get()
        .used_solve_time
        .with_label_values(&[solver, kind])
        .observe(compute_time.as_secs_f64());
}

/// Observe the result of mempool transaction execution.
pub fn mempool_executed(
    mempool: &Mempool,
    settlement: &Settlement,
    res: &Result<SubmissionSuccess, mempools::Error>,
) {
    match res {
        Ok(submission) => {
            tracing::info!(
                txid = ?submission.tx_hash,
                %mempool,
                ?settlement,
                "sending transaction via mempool succeeded",
            );
        }
        Err(mempools::Error::Disabled) => {
            tracing::debug!(
                %mempool,
                "sending transaction via mempool disabled",
            );
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                %mempool,
                ?settlement,
                "sending transaction via mempool failed",
            );
        }
    }
    let result = match res {
        Ok(_) => "Success",
        Err(mempools::Error::Revert { .. } | mempools::Error::SimulationRevert { .. }) => "Revert",
        Err(mempools::Error::Expired { .. }) => "Expired",
        Err(mempools::Error::Other(_)) => "Other",
        Err(mempools::Error::Disabled) => "Disabled",
    };
    metrics::get()
        .mempool_submission
        .with_label_values(&[&mempool.to_string(), result])
        .inc();

    // For some of the errors we are interested in observing the exact block numbers
    // passed since the first submission.
    let blocks_passed = match res {
        Ok(SubmissionSuccess {
            submitted_at_block,
            included_in_block,
            ..
        }) => Some(("Success", &submitted_at_block.0, &included_in_block.0)),
        Err(mempools::Error::Revert {
            tx_id: _,
            submitted_at_block,
            reverted_at_block,
        }) => Some(("Revert", submitted_at_block, reverted_at_block)),
        Err(mempools::Error::SimulationRevert {
            submitted_at_block,
            reverted_at_block,
        }) => Some(("Revert", submitted_at_block, reverted_at_block)),
        Err(mempools::Error::Expired {
            tx_id: _,
            submitted_at_block,
            submission_deadline,
        }) => Some(("Expired", submitted_at_block, submission_deadline)),
        Err(mempools::Error::Other(_)) => None,
        Err(mempools::Error::Disabled) => None,
    };

    if let Some((label, start, end)) = blocks_passed {
        let blocks_passed = end.saturating_sub(*start);
        metrics::get()
            .mempool_submission_results_blocks_passed
            .with_label_values(&[&mempool.to_string(), label])
            .inc_by(blocks_passed);
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
        competition::Error::DeadlineExceeded(_) => "DeadlineExceeded",
        competition::Error::Solver(solver::Error::Http(_)) => "SolverHttpError",
        competition::Error::Solver(solver::Error::Deserialize(_)) => "SolverDeserializeError",
        competition::Error::Solver(solver::Error::Dto(_)) => "SolverDtoError",
        competition::Error::SubmissionError => "SubmissionError",
        competition::Error::TooManyPendingSettlements => "TooManyPendingSettlements",
        competition::Error::NoValidOrdersFound => "NoValidOrdersFound",
        competition::Error::MalformedRequest => "MalformedRequest",
    }
}

pub fn deadline(deadline: &Deadline, timeouts: &Timeouts) {
    tracing::trace!(?deadline, ?timeouts, "computed deadline");
}

pub fn sending_solve_request(solver: &str, remaining_time: Duration, is_quote_request: bool) {
    tracing::trace!(?remaining_time, "sending solve request");
    let kind = if is_quote_request { "quote" } else { "auction" };
    metrics::get()
        .remaining_solve_time
        .with_label_values(&[solver, kind])
        .observe(remaining_time.as_secs_f64());
}

#[derive(Debug)]
pub enum OrderExcludedFromAuctionReason {
    CouldNotFetchBalance,
    InsufficientBalance,
    OrderWithZeroAmountRemaining,
}

pub fn order_excluded_from_auction(
    order: &competition::Order,
    reason: OrderExcludedFromAuctionReason,
) {
    tracing::trace!(uid=?order.uid, ?reason, "order excluded from auction");
}

/// Observe that a settlement was simulated
pub fn simulated(eth: &Ethereum, tx: &eth::Tx, gas: &Result<Gas, simulator::Error>) {
    let block: eth::BlockNo = eth.current_block().borrow().number.into();
    match gas {
        Ok(gas) => tracing::debug!(block = ?block, gas = ?gas.0, ?tx, "simulated settlement"),
        Err(err) => tracing::debug!(block = ?block, ?err, "simulated settlement"),
    }
}
