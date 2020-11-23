//! Module for common paths used for generated contract bindings.

use std::path::{Path, PathBuf};

/// Path to the directory containing the vendored contract artifacts.
pub fn contract_artifacts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts")
}
