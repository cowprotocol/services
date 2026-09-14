//! The Solana driver.

#![forbid(unsafe_code)]

pub mod domain;
pub mod infra;
mod run;

pub use self::run::{run, start};
