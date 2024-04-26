// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]
#![recursion_limit = "256"]

mod api;
mod boundary;
mod domain;
mod infra;
mod run;
#[cfg(test)]
mod tests;
mod util;

pub use self::run::{run, start};

pub use api::generate_openapi_yaml;