pub mod solve_request;
pub(crate) mod solve_response;

pub use {
    solve_request::{Error as AuctionError, SolveRequest},
    solve_response::SolveResponse,
};
