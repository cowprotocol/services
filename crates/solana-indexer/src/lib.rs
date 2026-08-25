//! `solana-indexer` — Solana settlement indexer.

#![warn(missing_docs)]

pub mod config;
pub mod indexer;
pub mod persistence;
pub mod run;
#[cfg(test)]
mod test_db;
pub mod types;
pub mod yellowstone;
