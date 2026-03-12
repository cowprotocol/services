use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct S3Config {
    /// The S3 bucket where auction instances should be uploaded.
    pub bucket: String,

    /// Prepended to the auction id to form the final instance filename on S3.
    /// Something like "staging/mainnet/"
    pub filename_prefix: String,
}

impl From<S3Config> for s3::Config {
    fn from(config: S3Config) -> Self {
        Self {
            bucket: config.bucket,
            filename_prefix: config.filename_prefix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full() {
        let toml = r#"
        bucket = "my-bucket"
        filename-prefix = "staging/mainnet/"
        "#;
        let config: S3Config = toml::from_str(toml).unwrap();
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.filename_prefix, "staging/mainnet/");
    }

    #[test]
    fn missing_field_fails() {
        let toml = r#"
        bucket = "my-bucket"
        "#;
        assert!(toml::from_str::<S3Config>(toml).is_err());
    }

    #[test]
    fn into_s3_config() {
        let config = S3Config {
            bucket: "my-bucket".to_string(),
            filename_prefix: "prefix/".to_string(),
        };
        let s3_config: s3::Config = config.into();
        assert_eq!(s3_config.bucket, "my-bucket");
        assert_eq!(s3_config.filename_prefix, "prefix/");
    }
}
