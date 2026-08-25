//! Shared test helpers for Solana CoW Protocol crates.

#![forbid(unsafe_code)]

use {
    solana_sdk::signer::keypair::{Keypair, write_keypair_file},
    tempfile::NamedTempFile,
};

/// Write a fresh keypair to a temp file and return the handle.
///
/// The file is destroyed when the returned handle is dropped, so callers
/// must keep it alive for as long as it's needed.
pub fn temp_keypair() -> NamedTempFile {
    let file = NamedTempFile::new().expect("create temp file");
    write_keypair_file(&Keypair::new(), file.path()).expect("write keypair");
    file
}
