//! The Solana driver.

#![forbid(unsafe_code)]

pub mod domain;
pub mod infra;
mod run;
pub mod util;

pub use self::run::{run, start};
