//! Wire DTOs for the `/solve` route.

mod solve_request;
mod solve_response;

pub use self::{
    solve_request::{Error as AuctionError, SolveRequest},
    solve_response::SolveResponse,
};
