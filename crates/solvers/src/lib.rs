// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]
#![recursion_limit = "256"]

mod api;
pub mod boundary;
pub mod domain;
mod infra;
mod run;
#[cfg(test)]
mod tests;
pub mod util;

pub use self::run::{run, start};
