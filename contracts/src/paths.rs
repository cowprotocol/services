//! Module for common paths used for generated contract bindings.

use std::path::{Path, PathBuf};

/// Path to file containing address of a contract deployed to a test network.
pub fn contract_address_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("deploy")
        .join(format!("{}.addr", name))
}

/// Path to the directory containing the vendored contract artifacts.
pub fn contract_artifacts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts")
}
