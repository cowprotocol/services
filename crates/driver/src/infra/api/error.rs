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
    SolutionNotAvailable,
    DeadlineExceeded,
    Unknown,
    InvalidAuctionId,
    MissingSurplusFee,
    InvalidTokens,
    InvalidAmounts,
    QuoteSameTokens,
    FailedToSubmit,
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
            competition::Error::SolutionNotAvailable => Kind::SolutionNotAvailable,
            competition::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            competition::Error::Solver(_) => Kind::SolverFailed,
            competition::Error::SubmissionError => Kind::FailedToSubmit,
        };
        error.into()
    }
}

impl From<api::routes::AuctionError> for (hyper::StatusCode, axum::Json<Error>) {
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

impl From<api::routes::OrderError> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: api::routes::OrderError) -> Self {
        let error = match value {
            api::routes::OrderError::SameTokens => Kind::QuoteSameTokens,
        };
        error.into()
    }
}
