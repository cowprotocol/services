//! Some variables are injected by 1Password directly into the Pod's
//! environment, completely bypassing Pulumi and making it unusable in the
//! configuration file. Since the token does not contain `%` this is a
//! workaround to allow the token to be read from the environment.
//!
//! [`Env`] does this for all types that implement [`FromStr`]. The functions
//! are the older, per-type form for `#[serde(deserialize_with = "...")]`.

use {
    serde::{Deserialize, Deserializer},
    std::{fmt::Display, str::FromStr},
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

/// A `T` parsed from *either* an environment variable — with the format
/// `%<ENV_VAR_NAME>` — or the field value. The field value must be a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Env<T>(pub T);

impl<T> Env<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<'de, T> Deserialize<'de> for Env<T>
where
    T: FromStr,
    T::Err: Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let value = match value.strip_prefix(ENV_VAR_PREFIX) {
            Some(env_var_name) => std::env::var(env_var_name)
                .inspect_err(|err| {
                    tracing::error!(%err, %env_var_name, "failed to load env var");
                })
                .map_err(|_| invalid_value_env_var_missing(env_var_name))?,
            None => value,
        };
        value.parse().map(Self).map_err(|err: T::Err| {
            serde::de::Error::custom(format_args!(
                "expected environment variable contents or passed field value to be a valid {}: \
                 {err}",
                std::any::type_name::<T>(),
            ))
        })
    }
}

/// Deserializes an URL from *either* an environment variable — with the format
/// `%<ENV_VAR_NAME>` — or interpreting a String as a URL.
pub fn deserialize_url_from_env<'de, D>(deserializer: D) -> Result<Url, D::Error>
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

/// Deserializes a String from *either* an environment variable —
/// with the format `%<ENV_VAR_NAME>` — or directly from the field value.
pub fn deserialize_string_from_env<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    match value.strip_prefix(ENV_VAR_PREFIX) {
        Some(env_var_name) => Ok(std::env::var(env_var_name)
            .inspect_err(|err| {
                tracing::error!(%err, %env_var_name, "failed to load env var");
            })
            .map_err(|_| invalid_value_env_var_missing(env_var_name))?),
        None => Ok(value),
    }
}

/// Deserializes an optional String from *either* an environment variable —
/// with the format `%<ENV_VAR_NAME>` — or directly from the field value.
/// Missing field or missing env var (when referenced) → `None`.
pub fn deserialize_optional_string_from_env<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    match value.strip_prefix(ENV_VAR_PREFIX) {
        Some(env_var_name) => Ok(std::env::var(env_var_name).ok()),
        None => Ok(Some(value)),
    }
}

/// Deserializes an optional URL from *either* an environment variable — with
/// the format `%<ENV_VAR_NAME>` — or interpreting a String as a URL.
pub fn deserialize_optional_url_from_env<'de, D>(deserializer: D) -> Result<Option<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(env_var_name) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let raw_url = match env_var_name.strip_prefix(ENV_VAR_PREFIX) {
        // In the case of optional variables, we assume a missing env var as
        // empty
        Some(env_var_name) => match std::env::var(env_var_name).ok() {
            Some(raw_url) => raw_url,
            None => return Ok(None),
        },

        None => env_var_name,
    };

    Ok(Some(
        Url::from_str(raw_url.as_str()).map_err(invalid_value_unable_to_parse_url)?,
    ))
}

#[cfg(test)]
mod tests {
    use {super::Env, serde::Deserialize, url::Url};

    #[derive(Deserialize)]
    struct Required {
        #[serde(deserialize_with = "super::deserialize_url_from_env")]
        url: Url,
    }

    #[derive(Deserialize)]
    struct Optional {
        #[serde(default, deserialize_with = "super::deserialize_optional_url_from_env")]
        url: Option<Url>,
    }

    #[derive(Deserialize)]
    struct Wrapped {
        urls: Vec<Env<Url>>,
        retries: Env<u32>,
    }

    #[test]
    fn wrapped_direct_and_from_env_var() {
        let url_var = "TEST_DESER_WRAPPED_URL_FROM_ENV";
        let retries_var = "TEST_DESER_WRAPPED_RETRIES_FROM_ENV";
        // Safety: test-only, using unique env var names to avoid conflicts
        unsafe { std::env::set_var(url_var, "http://example.com") };
        unsafe { std::env::set_var(retries_var, "3") };
        let json = format!(
            r#"{{"urls": ["http://localhost:8080", "%{url_var}"], "retries": "%{retries_var}"}}"#
        );
        let parsed: Wrapped = serde_json::from_str(&json).unwrap();
        let urls: Vec<_> = parsed.urls.iter().map(|url| url.0.as_str()).collect();
        assert_eq!(urls, ["http://localhost:8080/", "http://example.com/"]);
        assert_eq!(parsed.retries.into_inner(), 3);
        unsafe { std::env::remove_var(url_var) };
        unsafe { std::env::remove_var(retries_var) };
    }

    #[test]
    fn wrapped_missing_env_var_is_error() {
        let json = r#"{"urls": ["%NONEXISTENT_WRAPPED_TEST_VAR_12345"], "retries": "1"}"#;
        assert!(serde_json::from_str::<Wrapped>(json).is_err());
    }

    #[test]
    fn wrapped_invalid_value_is_error() {
        let json = r#"{"urls": [], "retries": "many"}"#;
        let err = serde_json::from_str::<Wrapped>(json).err().unwrap();
        assert!(err.to_string().contains("valid u32"), "{err}");
    }

    #[test]
    fn required_direct_url() {
        let json = r#"{"url": "http://localhost:8080"}"#;
        let parsed: Required = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.url.as_str(), "http://localhost:8080/");
    }

    #[test]
    fn required_from_env_var() {
        let var = "TEST_DESER_REQ_FROM_ENV";
        // Safety: test-only, using unique env var names to avoid conflicts
        unsafe { std::env::set_var(var, "http://example.com") };
        let json = format!(r#"{{"url": "%{var}"}}"#);
        let parsed: Required = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url.as_str(), "http://example.com/");
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn required_missing_env_var_is_error() {
        let json = r#"{"url": "%NONEXISTENT_TEST_VAR_12345"}"#;
        assert!(serde_json::from_str::<Required>(json).is_err());
    }

    #[test]
    fn required_invalid_url_is_error() {
        let json = r#"{"url": "not a url"}"#;
        assert!(serde_json::from_str::<Required>(json).is_err());
    }

    #[test]
    fn optional_none_when_absent() {
        let json = r#"{}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert!(parsed.url.is_none());
    }

    #[test]
    fn optional_none_when_null() {
        let json = r#"{"url": null}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert!(parsed.url.is_none());
    }

    #[test]
    fn optional_direct_url() {
        let json = r#"{"url": "http://localhost:9090"}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.url.unwrap().as_str(), "http://localhost:9090/");
    }

    #[test]
    fn optional_from_env_var() {
        let var = "TEST_DESER_OPT_FROM_ENV";
        unsafe { std::env::set_var(var, "http://opt.example.com") };
        let json = format!(r#"{{"url": "%{var}"}}"#);
        let parsed: Optional = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url.unwrap().as_str(), "http://opt.example.com/");
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn optional_missing_env_var_returns_none() {
        let json = r#"{"url": "%NONEXISTENT_OPT_TEST_VAR_12345"}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert!(parsed.url.is_none());
    }

    #[test]
    fn optional_invalid_url_is_error() {
        let json = r#"{"url": "not a url"}"#;
        assert!(serde_json::from_str::<Optional>(json).is_err());
    }
}
