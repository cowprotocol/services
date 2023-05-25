// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]
#![recursion_limit = "256"]

mod api;
pub mod boundary;
mod domain;
mod infra;
pub mod run;
#[cfg(test)]
mod tests;
mod util;
