use {
    crate::{
        domain::{eth, solver::legacy},
        infra::{self, config::unwrap_or_log, contracts},
        util::serialize,
    },
    reqwest::Url,
    s3::Uploader,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Chain id used to automatically determine the address
    /// of the WETH contract and for metrics.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// The solver name used in metrics.
    solver_name: String,

    /// The URL of the endpoint that responds to solve requests.
    endpoint: String,

    /// Enabled requests compression
    #[serde(default)]
    gzip_requests: bool,

    #[serde(default)]
    instance_upload: Option<S3>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct S3 {
    bucket: String,
    legacy_instance_prefix: String,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> legacy::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config = unwrap_or_log(toml::de::from_str::<Config>(&data), &path);
    let contracts = contracts::Contracts::for_chain(config.chain_id);

    let persistence = if let Some(s3) = config.instance_upload {
        let auction_uploader = Uploader::new(s3::Config {
            bucket: s3.bucket,
            filename_prefix: s3.legacy_instance_prefix,
        })
        .await;
        Some(infra::persistence::Persistence::new(auction_uploader))
    } else {
        None
    };

    legacy::Config {
        weth: contracts.weth,
        solver_name: config.solver_name,
        chain_id: config.chain_id,
        endpoint: Url::parse(&config.endpoint).unwrap(),
        gzip_requests: config.gzip_requests,
        persistence,
    }
}
