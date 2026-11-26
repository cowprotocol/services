use {
    serde::{Deserialize, Serialize},
    serde_ext::Env,
    url::Url,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IpfsConfig {
    /// IPFS gateway to fetch full app data for orders that only specify the
    /// contract app data hash.
    pub gateway: Url,

    /// Authentication key for Pinata IPFS gateway. A value like
    /// `%IPFS_PINATA_AUTH` reads the key from that environment variable.
    pub auth_token: Option<Env<String, true>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_token_is_redacted() {
        let config = IpfsConfig {
            gateway: Url::parse("https://google.com").unwrap(),
            auth_token: Some(Env("SOME_TOKEN".to_string())),
        };
        assert!(format!("{:?}", config).contains(r#"auth_token: Some("<REDACTED>")"#));
    }

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
        assert_eq!(config.auth_token.unwrap().into_inner(), "my-secret-key");
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
        assert_eq!(config.auth_token.unwrap().into_inner(), env_var_value);

        // SAFETY: no other threads access this env var.
        unsafe { std::env::remove_var(env_var_name) };
    }

    #[test]
    fn roundtrip_serialization() {
        let config = IpfsConfig {
            gateway: "https://gateway.pinata.cloud/ipfs/".parse().unwrap(),
            auth_token: Some(Env("my-secret-key".to_string())),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: IpfsConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.gateway.as_str(), deserialized.gateway.as_str());
        assert_eq!(config.auth_token, deserialized.auth_token);
    }
}
