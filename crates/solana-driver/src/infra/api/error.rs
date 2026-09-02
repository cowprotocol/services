//! The wire error shape shared by every API route.

use {
    crate::{
        domain::{auction, competition, settlement},
        infra::api::routes::AuctionError,
    },
    serde::Serialize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) enum Kind {
    InvalidAuctionId,
    SolverFailed,
    SolutionNotAvailable,
    DeadlineExceeded,
    TooManyPendingSettlements,
    FailedToSubmit,
    Unknown,
}

/// The wire error body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    kind: Kind,
    description: &'static str,
}

impl From<Kind> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(kind: Kind) -> Self {
        // TODO: Some of these kinds (e.g. SolverFailed, FailedToSubmit,
        // Unknown) would arguably be better as HTTP 500, but we deliberately
        // return 400 for every kind to match the EVM driver's status mapping.
        // Revisit once autopilot tooling no longer keys off the EVM contract.
        let description = match kind {
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            // Same wording as the EVM driver: an invalid or unproduced
            // solver response is the same kind of failure for autopilot.
            Kind::SolverFailed => "Solver engine returned an invalid response",
            Kind::SolutionNotAvailable => "The requested solution is not available",
            Kind::DeadlineExceeded => "The submission deadline has passed",
            Kind::TooManyPendingSettlements => "Too many settlements are pending",
            Kind::FailedToSubmit => "Failed to submit the settlement transaction",
            Kind::Unknown => "An unknown error occurred",
        };
        (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(Error { kind, description }),
        )
    }
}

impl From<auction::InvalidAuctionId> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(_: auction::InvalidAuctionId) -> Self {
        Kind::InvalidAuctionId.into()
    }
}

impl From<competition::Error> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: competition::Error) -> Self {
        match value {
            competition::Error::Solver(_) => Kind::SolverFailed,
            competition::Error::SolutionNotAvailable => Kind::SolutionNotAvailable,
            competition::Error::DeadlineExceeded => Kind::DeadlineExceeded,
            competition::Error::TooManyPendingSettlements => Kind::TooManyPendingSettlements,
            competition::Error::Rpc(_) => Kind::Unknown,
            competition::Error::FailedToSubmit(_) => Kind::FailedToSubmit,
            competition::Error::TaskPanicked => Kind::Unknown,
            // The solver is responsible for valid solutions. Map validation
            // errors to SolverFailed, as the EVM driver does. Map compile,
            // sign, or index-overflow errors to Unknown.
            competition::Error::Settlement(error) => match error {
                settlement::Error::Compile(_)
                | settlement::Error::Sign(_)
                | settlement::Error::InstructionIndexOverflow => Kind::Unknown,
                settlement::Error::NoTradeForOrder(_)
                | settlement::Error::NoOrderForTrade(_)
                | settlement::Error::ExecutedAmountOverflow
                | settlement::Error::NotExactlyFilled(_)
                | settlement::Error::Overfill(_)
                | settlement::Error::LimitPriceViolated(_)
                | settlement::Error::OrderPdaMismatch(..)
                | settlement::Error::OrderIntentMismatch(..) => Kind::SolverFailed,
                // The order expired between solve and settle: not solver
                // fault.
                settlement::Error::OrderExpired(_) => Kind::DeadlineExceeded,
            },
        }
        .into()
    }
}

impl From<AuctionError> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: AuctionError) -> Self {
        match value {
            AuctionError::InvalidAuctionId => Kind::InvalidAuctionId,
        }
        .into()
    }
}
