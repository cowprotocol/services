//! Solana-side autopilot components built on a chain-generic auction loop.

#![expect(dead_code, reason = "consumed by the binary wiring")]

pub mod run_loop;

mod domain;
mod infra;

#[cfg(test)]
mod test_db;
#[cfg(test)]
mod tests;
