//! Loading TOML configuration files into typed configs.

use {serde::de::DeserializeOwned, std::path::Path, tokio::fs};

/// Load a TOML configuration file into its typed form.
///
/// # Panics
///
/// Panics if the file cannot be read or does not parse into `C`. The parse
/// error itself is only printed with `TOML_TRACE_ERROR=1`, it may leak
/// secrets.
pub async fn load_toml<C: DeserializeOwned>(path: &Path) -> C {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));

    toml::de::from_str(&data).unwrap_or_else(|err| {
        if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
            panic!("failed to parse TOML config at {path:?}: {err:#?}")
        } else {
            panic!(
                "failed to parse TOML config at: {path:?}. Set TOML_TRACE_ERROR=1 to print \
                 parsing error but this may leak secrets."
            )
        }
    })
}
