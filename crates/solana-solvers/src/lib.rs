//! Solana solver engines for CoW Protocol.
//!
//! An MVP dex-wrapper over Jupiter, mirroring the `crates/solvers` shape over
//! Solana-native types (`u64`, `Pubkey`, instructions). Hosts the `/solve` API.

pub mod api;
mod cli;
pub mod config;
pub mod dex;
mod run;

pub use run::start;
