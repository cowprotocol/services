use {
    crate::{
        domain::{solver::naive, Risk},
        infra::config::unwrap_or_log,
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Parameters used to calculate the revert risk of a solution.
    /// (gas_amount_factor, gas_price_factor, nmb_orders_factor, intercept)
    risk_parameters: (f64, f64, f64, f64),
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> naive::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    // Not printing detailed error because it could potentially leak secrets.
    let config = unwrap_or_log(toml::de::from_str::<Config>(&data), &path);
    naive::Config {
        risk: Risk {
            gas_amount_factor: config.risk_parameters.0,
            gas_price_factor: config.risk_parameters.1,
            nmb_orders_factor: config.risk_parameters.2,
            intercept: config.risk_parameters.3,
        },
    }
}
