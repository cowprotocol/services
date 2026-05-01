use {
    crate::{
        domain::{competition, quote},
        infra::{api, blockchain},
    },
    serde::Serialize,
    solvers_dto,
    std::borrow::Cow,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
enum Kind {
    QuotingFailed,
    SolverFailed,
    TooManyPendingSettlements,
    SolutionNotAvailable,
    DeadlineExceeded,
    Unknown,
    InvalidAuctionId,
    MissingSurplusFee,
    InvalidTokens,
    InvalidAmounts,
    QuoteSameTokens,
    FailedToSubmit,
    NoValidOrders,
    MalformedRequest,
    TradingOutsideAllowedWindow,
    TokenTemporarilySuspended,
    InsufficientLiquidity,
    CustomSolverError,
    PreparationFailed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    kind: Kind,
    description: Cow<'static, str>,
}

impl From<Kind> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: Kind) -> Self {
        let description = match value {
            Kind::QuotingFailed => "No valid quote found",
            Kind::SolverFailed => "Solver engine returned an invalid response",
            Kind::SolutionNotAvailable => {
                "no solution is available yet, this might mean that /settle was called before \
                 /solve returned"
            }
            Kind::DeadlineExceeded => "Exceeded solution deadline",
            Kind::Unknown => "An unknown error occurred",
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            Kind::MissingSurplusFee => "Auction contains a limit order with no surplus fee",
            Kind::QuoteSameTokens => "Invalid quote with same buy and sell tokens",
            Kind::InvalidTokens => {
                "Invalid tokens specified in the auction, the tokens for some orders are missing"
            }
            Kind::InvalidAmounts => {
                "Invalid order specified in the auction, some orders have either a 0 remaining buy \
                 or sell amount"
            }
            Kind::FailedToSubmit => "Could not submit the solution to the blockchain",
            Kind::TooManyPendingSettlements => "Settlement queue is full",
            Kind::NoValidOrders => "No valid orders found in the auction",
            Kind::MalformedRequest => "Could not parse the request",
            Kind::TradingOutsideAllowedWindow => {
                "Token can only be traded during specific time windows"
            }
            Kind::TokenTemporarilySuspended => "Token is temporarily suspended from trading",
            Kind::InsufficientLiquidity => "Insufficient liquidity for the requested trade size",
            Kind::CustomSolverError => "Solver returned a custom error",
            Kind::PreparationFailed => "Preparation failed",
        };
        (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(Error {
                kind: value,
                description: description.into(),
            }),
        )
    }
}

fn map_custom_solver_error(custom_err: &solvers_dto::solution::SolverError) -> (Kind, String) {
    let (kind, default_message) = match custom_err.code {
        solvers_dto::solution::SolverErrorCode::TradingOutsideAllowedWindow => (
            Kind::TradingOutsideAllowedWindow,
            "Token can only be traded during specific time windows",
        ),
        solvers_dto::solution::SolverErrorCode::TokenTemporarilySuspended => (
            Kind::TokenTemporarilySuspended,
            "Token is temporarily suspended from trading",
        ),
        solvers_dto::solution::SolverErrorCode::InsufficientLiquidity => (
            Kind::InsufficientLiquidity,
            "Insufficient liquidity for the requested trade size",
        ),
        solvers_dto::solution::SolverErrorCode::Other => {
            (Kind::CustomSolverError, "Solver returned a custom error")
        }
    };

    let message = custom_err
        .message
        .clone()
        .unwrap_or_else(|| default_message.to_string());

    (kind, message)
}

impl From<quote::Error> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: quote::Error) -> Self {
        // Check if this is a custom solver error
        if let quote::Error::Solver(ref solver_err) = value
            && let Some(custom_err) = solver_err.custom_error()
        {
            tracing::warn!(err=?custom_err, "received custom solver error, mapping to internal errors");
            let (kind, description) = map_custom_solver_error(custom_err);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(Error {
                    kind,
                    description: description.into(),
                }),
            );
        }

        let error = match value {
            quote::Error::QuotingFailed(_) => Kind::QuotingFailed,
            quote::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            quote::Error::Solver(_) => Kind::SolverFailed,
            quote::Error::Blockchain(_) => Kind::Unknown,
            quote::Error::Boundary(_) => Kind::Unknown,
            quote::Error::Encoding(_) => Kind::Unknown,
        };
        error.into()
    }
}

impl From<competition::Error> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: competition::Error) -> Self {
        let error = match value {
            competition::Error::SolutionNotAvailable => Kind::SolutionNotAvailable,
            competition::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            competition::Error::Solver(_) => Kind::SolverFailed,
            competition::Error::SubmissionError => Kind::FailedToSubmit,
            competition::Error::TooManyPendingSettlements => Kind::TooManyPendingSettlements,
            competition::Error::NoValidOrdersFound => Kind::NoValidOrders,
            competition::Error::MalformedRequest => Kind::MalformedRequest,
            competition::Error::PreparationError => Kind::PreparationFailed,
        };
        error.into()
    }
}

impl From<blockchain::Error> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(_: blockchain::Error) -> Self {
        Kind::Unknown.into()
    }
}

impl From<api::routes::AuctionError> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::AuctionError) -> Self {
        let error = match value {
            api::routes::AuctionError::InvalidAuctionId => Kind::InvalidAuctionId,
            api::routes::AuctionError::MissingSurplusFee => Kind::MissingSurplusFee,
            api::routes::AuctionError::InvalidTokens => Kind::InvalidTokens,
            api::routes::AuctionError::InvalidAmounts => Kind::InvalidAmounts,
            api::routes::AuctionError::Blockchain(_) => Kind::Unknown,
        };
        error.into()
    }
}

impl From<api::routes::OrderError> for (axum::http::StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::OrderError) -> Self {
        let error = match value {
            api::routes::OrderError::SameTokens => Kind::QuoteSameTokens,
        };
        error.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_custom_solver_error_codes_to_kinds_and_default_messages() {
        let cases = [
            (
                solvers_dto::solution::SolverErrorCode::TradingOutsideAllowedWindow,
                Kind::TradingOutsideAllowedWindow,
                "Token can only be traded during specific time windows",
            ),
            (
                solvers_dto::solution::SolverErrorCode::TokenTemporarilySuspended,
                Kind::TokenTemporarilySuspended,
                "Token is temporarily suspended from trading",
            ),
            (
                solvers_dto::solution::SolverErrorCode::InsufficientLiquidity,
                Kind::InsufficientLiquidity,
                "Insufficient liquidity for the requested trade size",
            ),
            (
                solvers_dto::solution::SolverErrorCode::Other,
                Kind::CustomSolverError,
                "Solver returned a custom error",
            ),
        ];

        for (code, expected_kind, expected_message) in cases {
            let custom_err = solvers_dto::solution::SolverError {
                code,
                message: None,
            };

            let (kind, message) = map_custom_solver_error(&custom_err);
            assert_eq!(kind, expected_kind);
            assert_eq!(message, expected_message);
        }
    }

    #[test]
    fn preserves_custom_solver_error_message_when_provided() {
        let custom_err = solvers_dto::solution::SolverError {
            code: solvers_dto::solution::SolverErrorCode::Other,
            message: Some("downstream solver reason".to_string()),
        };

        let (kind, message) = map_custom_solver_error(&custom_err);
        assert!(matches!(kind, Kind::CustomSolverError));
        assert_eq!(message, "downstream solver reason");
    }
}
