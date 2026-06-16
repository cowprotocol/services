use {serde::Deserialize, std::path::Path, tokio::fs};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub chain_id: u64,
}

pub async fn load(path: &Path) -> Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|e| panic!("invalid BYOS config {path:?}: {e:?}"))
}
