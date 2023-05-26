use {
    crate::{
        domain::{competition, quote},
        infra::api,
    },
    serde::Serialize,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "PascalCase")]
enum Kind {
    QuotingFailed,
    SolverFailed,
    InvalidSolutionId,
    SolutionNotFound,
    DeadlineExceeded,
    Unknown,
    TransactionPublishingFailed,
    InvalidAuctionId,
    MissingSurplusFee,
    QuoteSameTokens,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    kind: Kind,
    description: &'static str,
}

impl From<Kind> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: Kind) -> Self {
        let description = match value {
            Kind::QuotingFailed => "No valid quote found",
            Kind::SolverFailed => "Solver engine returned an invalid response",
            Kind::SolutionNotFound => "No solution found for the auction",
            Kind::InvalidSolutionId => "No solution found for the given ID",
            Kind::DeadlineExceeded => "Exceeded solution deadline",
            Kind::Unknown => "An unknown error occurred",
            Kind::TransactionPublishingFailed => "Failed to publish the settlement transaction",
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            Kind::MissingSurplusFee => "Auction contains a limit order with no surplus fee",
            Kind::QuoteSameTokens => "Invalid quote with same buy and sell tokens",
        };
        (
            hyper::StatusCode::BAD_REQUEST,
            axum::Json(Error {
                kind: value,
                description,
            }),
        )
    }
}

impl From<quote::Error> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: quote::Error) -> Self {
        let error = match value {
            quote::Error::QuotingFailed(_) => Kind::QuotingFailed,
            quote::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            quote::Error::Solver(_) => Kind::SolverFailed,
            quote::Error::Blockchain(_) => Kind::Unknown,
            quote::Error::Boundary(_) => Kind::Unknown,
        };
        error.into()
    }
}

impl From<competition::Error> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: competition::Error) -> Self {
        let error = match value {
            competition::Error::InvalidSolutionId => Kind::InvalidSolutionId,
            competition::Error::SolutionNotFound => Kind::SolutionNotFound,
            competition::Error::Mempool(_) => Kind::TransactionPublishingFailed,
            competition::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            competition::Error::Solver(_) => Kind::SolverFailed,
        };
        error.into()
    }
}

impl From<api::routes::AuctionError> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::AuctionError) -> Self {
        let error = match value {
            api::routes::AuctionError::InvalidAuctionId => Kind::InvalidAuctionId,
            api::routes::AuctionError::MissingSurplusFee => Kind::MissingSurplusFee,
            api::routes::AuctionError::GasPrice(_) => Kind::Unknown,
        };
        error.into()
    }
}

impl From<api::routes::OrderError> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::OrderError) -> Self {
        let error = match value {
            api::routes::OrderError::SameTokens => Kind::QuoteSameTokens,
        };
        error.into()
    }
}
