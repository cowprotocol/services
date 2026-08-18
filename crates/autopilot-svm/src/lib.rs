//! Solana-side autopilot components built on a chain-generic auction loop.

pub mod run_loop;

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the auction loop wiring")
)]
mod infra;
