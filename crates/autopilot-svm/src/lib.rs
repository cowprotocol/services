//! Solana-side autopilot components built on a chain-generic auction loop.

// No binary target consumes these modules yet, so parts of them are dead code
// by construction. The `expect` starts warning the moment the wiring makes
// them live, forcing its own removal.
#![expect(
    dead_code,
    reason = "no binary target yet, the wiring PR consumes these modules"
)]

mod infra;
mod run_loop;
