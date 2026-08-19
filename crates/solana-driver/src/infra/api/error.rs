//! The wire error shape shared by every API route.

use {
    crate::{
        domain::{auction, competition},
        infra::api::routes::AuctionError,
    },
    serde::Serialize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) enum Kind {
    InvalidAuctionId,
    SolverFailed,
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
        let description = match kind {
            Kind::InvalidAuctionId => "Invalid ID specified in the auction",
            Kind::SolverFailed => "Solver engine returned an invalid response",
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
