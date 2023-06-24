use {serde::Deserialize, url::Url};

/// Load the prices service configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &std::path::Path) -> Config {
    let file = tokio::fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    toml::de::from_str(&file)
        .unwrap_or_else(|e| panic!("Configuration error while reading {path:?}: {e:?}"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub timeout_ms: u64,
    pub zeroex: Option<Zeroex>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Zeroex {
    pub api_key: Option<String>,
    pub endpoint: Option<Url>,
    pub enable: bool,
}
