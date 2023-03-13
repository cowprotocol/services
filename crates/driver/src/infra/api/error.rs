use {
    crate::{
        domain::{
            competition::{self, solution},
            quote,
        },
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
    QuoteSameTokens,
    InvalidAssetFlow,
    InvalidInternalization,
    MissingWeth,
    InsufficientBalance,
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
            Kind::SolutionNotFound => "No solution found for given ID",
            Kind::DeadlineExceeded => "Exceeded solution deadline",
            Kind::SimulationFailed => "Solution simulation failed",
            Kind::Unknown => "An unknown error occurred",
            Kind::TransactionPublishingFailed => "Failed to publish the settlement transaction",
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            Kind::MissingSurplusFee => "Auction contains a limit order with no surplus fee",
            Kind::QuoteSameTokens => "Invalid quote with same buy and sell tokens",
            Kind::InvalidAssetFlow => {
                "The solver returned a solution with invalid asset flow: token amounts entering \
                 the settlement contract are lower than token amounts exiting the contract"
            }
            Kind::InvalidInternalization => {
                "The solver returned a solution which internalizes interactions with untrusted \
                 tokens"
            }
            Kind::MissingWeth => "missing WETH clearing price",
            Kind::InsufficientBalance => "Solver has insufficient Ether balance",
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
            quote::Error::QuotingFailed => Kind::QuotingFailed,
            quote::Error::DeadlineExceeded(_) => Kind::DeadlineExceeded,
            quote::Error::Solver(_) => Kind::SolverFailed,
            quote::Error::Boundary(_) => Kind::Unknown,
        };
        error.into()
    }
}

impl From<competition::Error> for (hyper::StatusCode, axum::Json<Error>) {
    fn from(value: competition::Error) -> Self {
        let error = match value {
            competition::Error::SolutionNotFound => Kind::SolutionNotFound,
            competition::Error::Solution(solution::Error::Blockchain(_)) => Kind::Unknown,
            competition::Error::Solution(solution::Error::Boundary(_)) => Kind::Unknown,
            competition::Error::Solution(solution::Error::Verification(
                solution::VerificationError::Simulation(_),
            )) => Kind::SimulationFailed,
            competition::Error::Solution(solution::Error::Verification(
                solution::VerificationError::AssetFlow,
            )) => Kind::InvalidAssetFlow,
            competition::Error::Solution(solution::Error::Verification(
                solution::VerificationError::Internalization,
            )) => Kind::InvalidInternalization,
            competition::Error::Solution(solution::Error::MissingWethClearingPrice) => {
                Kind::MissingWeth
            }
            competition::Error::Solution(solution::Error::InsufficientBalance) => {
                Kind::InsufficientBalance
            }
            competition::Error::Mempool(_) => Kind::TransactionPublishingFailed,
            competition::Error::Boundary(_) => Kind::Unknown,
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
