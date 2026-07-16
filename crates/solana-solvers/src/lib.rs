//! Solana solver engines for CoW Protocol.
//!
//! An MVP dex-wrapper over Jupiter's quote API, mirroring the `crates/solvers`
//! shape over Solana-native types. This crate is the HTTP `/solve` host; the
//! Jupiter adapter, solution assembly, and solve loop land in later PRs, and a
//! Solana baseline engine joins once the driver's liquidity module unfreezes.

pub mod api;
mod cli;
pub mod config;
mod run;

pub use run::start;
