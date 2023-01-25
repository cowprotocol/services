use {
    crate::{
        domain::competition::{self, quote, solution},
        infra::api,
    },
    hyper::StatusCode,
    serde::Serialize,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "PascalCase")]
enum Type {
    QuotingFailed,
    SolverFailed,
    SolutionNotFound,
    DeadlineExceeded,
    SimulationFailed,
    Unknown,
    TransactionPublishingFailed,
    InvalidAuctionId,
    MissingSurplusFee,
    LiquidityError,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    error_type: Type,
    description: &'static str,
}

impl From<Type> for (StatusCode, axum::Json<Error>) {
    fn from(value: Type) -> Self {
        let (description, status_code) = match value {
            Type::QuotingFailed => ("No valid quote found", StatusCode::BAD_REQUEST),
            Type::SolverFailed => (
                "Solver engine returned an invalid response",
                StatusCode::BAD_REQUEST,
            ),
            Type::SolutionNotFound => ("No solution found for given ID", StatusCode::BAD_REQUEST),
            Type::DeadlineExceeded => ("Exceeded solution deadline", StatusCode::BAD_REQUEST),
            Type::SimulationFailed => ("Solution simulation failed", StatusCode::BAD_REQUEST),
            Type::Unknown => ("An unknown error occurred", StatusCode::BAD_REQUEST),
            Type::TransactionPublishingFailed => (
                "Failed to publish the settlement transaction",
                StatusCode::BAD_REQUEST,
            ),
            Type::InvalidAuctionId => (
                "Invalid ID specified in the auction",
                StatusCode::BAD_REQUEST,
            ),
            Type::MissingSurplusFee => (
                "Auction contains a limit order with no surplus fee",
                StatusCode::BAD_REQUEST,
            ),
            Type::LiquidityError => ("Failed to fetch onchain liquidity", StatusCode::BAD_REQUEST),
        };
        (
            status_code,
            axum::Json(Error {
                error_type: value,
                description,
            }),
        )
    }
}

impl From<quote::Error> for (StatusCode, axum::Json<Error>) {
    fn from(value: quote::Error) -> Self {
        let error = match value {
            quote::Error::QuotingFailed => Type::QuotingFailed,
            quote::Error::Solver(_) => Type::SolverFailed,
        };
        error.into()
    }
}

impl From<competition::Error> for (StatusCode, axum::Json<Error>) {
    fn from(value: competition::Error) -> Self {
        let error = match value {
            competition::Error::SolutionNotFound => Type::SolutionNotFound,
            competition::Error::Solution(solution::Error::Simulation(_)) => Type::SimulationFailed,
            competition::Error::Solution(solution::Error::Blockchain(_)) => Type::Unknown,
            competition::Error::Solution(solution::Error::Boundary(_)) => Type::Unknown,
            competition::Error::Mempool(_) => Type::TransactionPublishingFailed,
            competition::Error::Boundary(_) => Type::Unknown,
            competition::Error::DeadlineExceeded(_) => Type::DeadlineExceeded,
            competition::Error::Solver(_) => Type::SolverFailed,
        };
        error.into()
    }
}

impl From<api::routes::AuctionError> for (StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::AuctionError) -> Self {
        let error = match value {
            api::routes::AuctionError::InvalidAuctionId => Type::InvalidAuctionId,
            api::routes::AuctionError::MissingSurplusFee => Type::MissingSurplusFee,
            api::routes::AuctionError::DeadlineExceeded => Type::DeadlineExceeded,
            api::routes::AuctionError::Liquidity(_) => Type::LiquidityError,
        };
        error.into()
    }
}
