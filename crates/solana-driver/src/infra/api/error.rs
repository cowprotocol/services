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
    InvalidSolution,
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
        let (status, description) = match kind {
            Kind::InvalidAuctionId => (
                axum::http::StatusCode::BAD_REQUEST,
                "Invalid ID specified in the auction",
            ),
            Kind::SolverFailed => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "All solver engines failed to produce solutions",
            ),
            Kind::SolutionNotAvailable => (
                axum::http::StatusCode::BAD_REQUEST,
                "The requested solution is not available",
            ),
            Kind::InvalidSolution => (
                axum::http::StatusCode::BAD_REQUEST,
                "The solution failed the driver's validation",
            ),
            Kind::DeadlineExceeded => (
                axum::http::StatusCode::BAD_REQUEST,
                "The submission deadline has passed",
            ),
            Kind::TooManyPendingSettlements => (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Too many settlements are pending",
            ),
            Kind::FailedToSubmit => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to submit the settlement transaction",
            ),
            Kind::Unknown => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "An unknown error occurred",
            ),
        };
        (status, axum::Json(Error { kind, description }))
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
            // A solution failing validation is the solver's fault; a
            // settlement that validated but could not be compiled or signed
            // is not.
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
                | settlement::Error::OrderExpired(_)
                | settlement::Error::OrderPdaMismatch(..)
                | settlement::Error::OrderIntentMismatch(..) => Kind::InvalidSolution,
            },
            competition::Error::Prepare(error) => match error {
                // The solver supplied the lookup table keys.
                settlement::PrepareError::InvalidAddressLookupTable { .. } => Kind::InvalidSolution,
                // Neither the solver nor the driver controls RPC failures or
                // a foreign-owned setup account; nothing was submitted.
                settlement::PrepareError::Rpc(_)
                | settlement::PrepareError::UnexpectedSetupAccount { .. } => Kind::Unknown,
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
