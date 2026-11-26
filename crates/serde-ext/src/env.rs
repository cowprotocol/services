//! Some variables are injected by 1Password directly into the Pod's
//! environment, completely bypassing Pulumi and making it unusable in the
//! configuration file. Since the token does not contain `%` this is a
//! workaround to allow the token to be read from the environment.

use {
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{
        fmt::{self, Debug, Display},
        ops::Deref,
        str::FromStr,
    },
};

const ENV_VAR_PREFIX: char = '%';

/// A `T` parsed from *either* an environment variable — with the format
/// `%<ENV_VAR_NAME>` — or the field value. The field value must be a string.
/// `Env<T, true>` keeps the value out of the [`Debug`] output.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Env<T, const REDACTED: bool = false>(pub T);

impl<T, const REDACTED: bool> Env<T, REDACTED> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, const REDACTED: bool> Env<T, REDACTED>
where
    T: FromStr,
    T::Err: Display,
{
    /// Deserializes an optional [`Env`] for `#[serde(deserialize_with)]`.
    /// Missing field or missing env var (when referenced) → `None`.
    pub fn deserialize_optional<'de, D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(value) = Option::<String>::deserialize(deserializer)? else {
            return Ok(None);
        };
        let value = match value.strip_prefix(ENV_VAR_PREFIX) {
            // In the case of optional variables, we assume a missing env var as
            // empty
            Some(env_var_name) => match std::env::var(env_var_name).ok() {
                Some(value) => value,
                None => return Ok(None),
            },
            None => value,
        };
        Self::parse(&value).map(Some)
    }

    /// Parses the environment variable contents or the field value.
    fn parse<E: serde::de::Error>(value: &str) -> Result<Self, E> {
        value.parse().map(Self).map_err(|err: T::Err| {
            E::custom(format_args!(
                "expected environment variable contents or passed field value to be a valid {}: \
                 {err}",
                std::any::type_name::<T>(),
            ))
        })
    }
}

impl<T, const REDACTED: bool> From<T> for Env<T, REDACTED> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T, const REDACTED: bool> Deref for Env<T, REDACTED> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Debug, const REDACTED: bool> Debug for Env<T, REDACTED> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match REDACTED {
            true => Debug::fmt("<REDACTED>", f),
            false => self.0.fmt(f),
        }
    }
}

impl<'de, T, const REDACTED: bool> Deserialize<'de> for Env<T, REDACTED>
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
            Some(env_var_name) => std::env::var(env_var_name).map_err(|err| {
                serde::de::Error::custom(format_args!(
                    "failed to load environment variable {env_var_name}: {err}"
                ))
            })?,
            None => value,
        };
        Self::parse(&value)
    }
}

/// Writes the value itself, also for `Env<T, true>`, so that the output can be
/// deserialized again.
impl<T: Display, const REDACTED: bool> Serialize for Env<T, REDACTED> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use {super::Env, serde::Deserialize, url::Url};

    #[derive(Debug, Deserialize)]
    struct Required {
        url: Env<Url>,
    }

    #[derive(Debug, Deserialize)]
    struct Optional {
        #[serde(default, deserialize_with = "Env::deserialize_optional")]
        url: Option<Env<Url, true>>,
    }

    #[test]
    fn required_direct_value() {
        let json = r#"{"url": "http://localhost:8080"}"#;
        let parsed: Required = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.url.as_str(), "http://localhost:8080/");
    }

    #[test]
    fn required_from_env_var() {
        let var = "TEST_SERDE_EXT_ENV_REQUIRED";
        // Safety: test-only, using unique env var names to avoid conflicts
        unsafe { std::env::set_var(var, "http://example.com") };
        let json = format!(r#"{{"url": "%{var}"}}"#);
        let parsed: Required = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url.as_str(), "http://example.com/");
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn required_missing_env_var_is_error() {
        let json = r#"{"url": "%NONEXISTENT_SERDE_EXT_ENV_REQUIRED"}"#;
        let err = serde_json::from_str::<Required>(json).unwrap_err();
        assert!(
            err.to_string()
                .contains("NONEXISTENT_SERDE_EXT_ENV_REQUIRED"),
            "{err}"
        );
    }

    #[test]
    fn required_invalid_value_is_error() {
        let err = serde_json::from_str::<Env<u32>>(r#""many""#).unwrap_err();
        assert!(err.to_string().contains("valid u32"), "{err}");
    }

    #[test]
    fn composes_with_containers() {
        let var = "TEST_SERDE_EXT_ENV_CONTAINER";
        unsafe { std::env::set_var(var, "http://example.com") };
        let json = format!(r#"["http://localhost:8080", "%{var}"]"#);
        let parsed: Vec<Env<Url>> = serde_json::from_str(&json).unwrap();
        let urls: Vec<_> = parsed.iter().map(|url| url.as_str()).collect();
        assert_eq!(urls, ["http://localhost:8080/", "http://example.com/"]);
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn optional_none_when_absent_or_null() {
        let parsed: Optional = serde_json::from_str(r#"{}"#).unwrap();
        assert!(parsed.url.is_none());
        let parsed: Optional = serde_json::from_str(r#"{"url": null}"#).unwrap();
        assert!(parsed.url.is_none());
    }

    #[test]
    fn optional_direct_value() {
        let json = r#"{"url": "http://localhost:9090"}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.url.unwrap().as_str(), "http://localhost:9090/");
    }

    #[test]
    fn optional_from_env_var() {
        let var = "TEST_SERDE_EXT_ENV_OPTIONAL";
        unsafe { std::env::set_var(var, "http://opt.example.com") };
        let json = format!(r#"{{"url": "%{var}"}}"#);
        let parsed: Optional = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url.unwrap().as_str(), "http://opt.example.com/");
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn optional_missing_env_var_returns_none() {
        let json = r#"{"url": "%NONEXISTENT_SERDE_EXT_ENV_OPTIONAL"}"#;
        let parsed: Optional = serde_json::from_str(json).unwrap();
        assert!(parsed.url.is_none());
    }

    #[test]
    fn optional_invalid_value_is_error() {
        let json = r#"{"url": "not a url"}"#;
        assert!(serde_json::from_str::<Optional>(json).is_err());
    }

    #[test]
    fn debug_redacts_on_request() {
        let plain: Env<String> = Env("visible".to_string());
        let secret: Env<String, true> = Env("hidden".to_string());
        assert_eq!(format!("{plain:?}"), r#""visible""#);
        assert_eq!(format!("{:?}", Some(secret)), r#"Some("<REDACTED>")"#);
    }

    #[test]
    fn serializes_the_value() {
        let secret: Env<Url, true> = Env("http://localhost:8080".parse().unwrap());
        assert_eq!(
            serde_json::to_string(&secret).unwrap(),
            r#""http://localhost:8080/""#
        );
    }
}
