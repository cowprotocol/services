use {
    serde::{Deserialize, Deserializer, Serialize},
    url::Url,
};

/// The IPFS authentication token is injected by 1Password directly into the
/// Pod's environment, completely bypassing Pulumi and making it unusable in the
/// configuration file. Since the token does not contain `%` this is a
/// workaround to allow the token to be read from the environment.
fn deserialize_auth_token<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(raw_auth_token) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    if raw_auth_token.starts_with("%") {
        let env_var_name = &raw_auth_token[1..];
        let env_var_contents = std::env::var(env_var_name).map_err(|err| {
            tracing::error!(%err, %env_var_name, "failed to load env var");
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(env_var_name),
                &"expected environment variable to be available",
            )
        })?;
        Ok(Some(env_var_contents))
    } else {
        Ok(Some(raw_auth_token))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IpfsConfig {
    /// IPFS gateway to fetch full app data for orders that only specify the
    /// contract app data hash.
    pub gateway: Url,

    /// Authentication key for Pinata IPFS gateway.
    #[serde(deserialize_with = "deserialize_auth_token")]
    pub auth_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full() {
        let toml = r#"
        gateway = "https://gateway.pinata.cloud/ipfs/"
        auth-token = "my-secret-key"
        "#;
        let config: IpfsConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.gateway.as_str(),
            "https://gateway.pinata.cloud/ipfs/"
        );
        assert_eq!(config.auth_token.unwrap(), "my-secret-key");
    }

    #[test]
    fn deserialize_auth_token_from_env() {
        let env_var_name = "TEST_IPFS_AUTH_TOKEN_SECRET";
        let env_var_value = "my-secret-from-env";
        // SAFETY: no other threads access this env var.
        unsafe { std::env::set_var(env_var_name, env_var_value) };

        let toml = format!(
            r#"
            gateway = "https://gateway.pinata.cloud/ipfs/"
            auth-token = "%{env_var_name}"
            "#,
        );
        let config: IpfsConfig = toml::from_str(&toml).unwrap();
        assert_eq!(config.auth_token.as_deref(), Some(env_var_value));

        // SAFETY: no other threads access this env var.
        unsafe { std::env::remove_var(env_var_name) };
    }

    #[test]
    fn roundtrip_serialization() {
        let config = IpfsConfig {
            gateway: "https://gateway.pinata.cloud/ipfs/".parse().unwrap(),
            auth_token: Some("my-secret-key".to_string()),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: IpfsConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.gateway.as_str(), deserialized.gateway.as_str());
        assert_eq!(config.auth_token, deserialized.auth_token);
    }
}
