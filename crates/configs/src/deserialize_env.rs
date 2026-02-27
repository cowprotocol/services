//! Some variables are injected by 1Password directly into the Pod's
//! environment, completely bypassing Pulumi and making it unusable in the
//! configuration file. Since the token does not contain `%` this is a
//! workaround to allow the token to be read from the environment.
//!
//! Due to handling several kinds of types

use {
    serde::{Deserialize, Deserializer},
    std::str::FromStr,
    url::{ParseError, Url},
};

const ENV_VAR_PREFIX: char = '%';

/// Returns a deserialization error mentioning that the target environment
/// variable could not be found.
fn invalid_value_env_var_missing<E: serde::de::Error>(var_name: &str) -> E {
    serde::de::Error::invalid_value(
        serde::de::Unexpected::Str(var_name),
        &"expected environment variable to be available",
    )
}

/// Returns a deserialization error mentioning that either the environment
/// variable contents or the field value is not a valid URL.
fn invalid_value_unable_to_parse_url<E: serde::de::Error>(err: ParseError) -> E {
    serde::de::Error::invalid_value(
        serde::de::Unexpected::Other(err.to_string().as_str()),
        &"expected environment variable contents or passed field value to be a valid URL",
    )
}

/// Deserializes an URL from *either* an environment variable — with the format
/// `%<ENV_VAR_NAME>` — or interpreting a String as a URL.
pub(crate) fn deserialize_url_from_env<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let env_var_name = String::deserialize(deserializer)?;

    let raw_url = match env_var_name.strip_prefix(ENV_VAR_PREFIX) {
        Some(env_var_name) => std::env::var(env_var_name)
            .inspect_err(|err| {
                tracing::error!(%err, %env_var_name, "failed to load env var");
            })
            .map_err(|_| invalid_value_env_var_missing(env_var_name))?,
        None => env_var_name,
    };

    Url::from_str(&raw_url).map_err(invalid_value_unable_to_parse_url)
}

/// Deserializes an optional URL from *either* an environment variable — with
/// the format `%<ENV_VAR_NAME>` — or interpreting a String as a URL.
pub(crate) fn deserialize_optional_url_from_env<'de, D>(
    deserializer: D,
) -> Result<Option<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(env_var_name) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let raw_url = match env_var_name.strip_prefix(ENV_VAR_PREFIX) {
        Some(env_var_name) => std::env::var(env_var_name)
            .inspect_err(|err| {
                tracing::error!(%err, %env_var_name, "failed to load env var");
            })
            .map_err(|_| invalid_value_env_var_missing(env_var_name))?,
        None => env_var_name,
    };

    Ok(Some(
        Url::from_str(raw_url.as_str()).map_err(invalid_value_unable_to_parse_url)?,
    ))
}
