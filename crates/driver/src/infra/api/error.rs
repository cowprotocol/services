use {
    crate::{
        domain::competition::{self, quote, solution},
        infra::api,
    },
    serde::Serialize,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "PascalCase")]
enum Kind {
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
    kind: Kind,
    description: &'static str,
}

impl From<Kind> for axum::Json<Error> {
    fn from(value: Kind) -> Self {
        let description = match value {
            Kind::QuotingFailed => "No valid quote found",
            Kind::SolverFailed => "Solver engine returned an invalid response",
            Kind::SolutionNotFound => "No solution found for given ID",
            Kind::DeadlineExceeded => "Exceeded solution deadline",
            Kind::SimulationFailed => "Solution simulation failed",
            Kind::Unknown => "An unknown error occurred",
            Kind::TransactionPublishingFailed => "Failed to publish the settlement transaction",
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            Kind::MissingSurplusFee => "Auction contains a limit order with no surplus fee",
            Kind::LiquidityError => "Failed to fetch onchain liquidity",
        };
        axum::Json(Error {
            kind: value,
            description,
        })
    }
}

impl From<quote::Error> for axum::Json<Error> {
    fn from(value: quote::Error) -> Self {
        let error = match value {
            quote::Error::QuotingFailed => Kind::QuotingFailed,
            quote::Error::Solver(_) => Kind::SolverFailed,
        };
        error.into()
    }
}

impl From<competition::Error> for axum::Json<Error> {
    fn from(value: competition::Error) -> Self {
        let error = match value {
            competition::Error::SolutionNotFound => Kind::SolutionNotFound,
            competition::Error::Solution(solution::Error::Simulation(_)) => Kind::SimulationFailed,
            competition::Error::Solution(solution::Error::Blockchain(_)) => Kind::Unknown,
            competition::Error::Solution(solution::Error::Boundary(_)) => Kind::Unknown,
            competition::Error::Mempool(_) => Kind::TransactionPublishingFailed,
            competition::Error::Boundary(_) => Kind::Unknown,
            competition::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            competition::Error::Solver(_) => Kind::SolverFailed,
        };
        error.into()
    }
}

impl From<api::routes::AuctionError> for axum::Json<Error> {
    fn from(value: api::routes::AuctionError) -> Self {
        let error = match value {
            api::routes::AuctionError::InvalidAuctionId => Kind::InvalidAuctionId,
            api::routes::AuctionError::MissingSurplusFee => Kind::MissingSurplusFee,
            api::routes::AuctionError::DeadlineExceeded => Kind::DeadlineExceeded,
            api::routes::AuctionError::Liquidity(_) => Kind::LiquidityError,
        };
        error.into()
    }
}
