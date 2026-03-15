//! This is a library so that it can be used from e2e tests without having to
//! spawn a process.

#![forbid(unsafe_code)]
#![feature(duration_constructors_lite)]
#![feature(slice_as_array)]

pub mod boundary;
pub mod domain;
pub mod infra;
mod run;
pub mod util;

#[cfg(test)]
mod tests;

pub use self::run::{run, start};

pub use model;
pub use app_data;
pub use observe;
pub use ethrpc;
pub use serde_ext;
pub use solvers_dto;
pub use shared;