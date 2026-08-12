//! Solana-side autopilot components built on a chain-generic auction loop.

#![expect(
    dead_code,
    reason = "no binary target yet, the wiring PR consumes these modules"
)]

mod auction;
mod infra;
mod run_loop;
