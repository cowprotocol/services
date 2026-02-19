use {
    crate::config::{fee_policy::FeePoliciesConfig, solver::Solver},
    anyhow::{anyhow, ensure},
    serde::{Deserialize, Serialize},
    std::path::Path,
};

pub mod fee_policy;
pub mod solver;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    // #[serde(default)]
    pub drivers: Vec<Solver>,

    /// Describes how the protocol fees should be calculated.
    #[serde(flatten)]
    pub fee_policies_config: FeePoliciesConfig,
}

impl Configuration {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        match toml::from_str(&tokio::fs::read_to_string(&path).await?) {
            Ok(self_) => Ok(self_),
            Err(err) if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") => Err(anyhow!(
                "failed to parse TOML config at {}: {err:#?}",
                path.as_ref().display()
            )),
            Err(_) => Err(anyhow!(
                "failed to parse TOML config at: {}. Set TOML_TRACE_ERROR=1 to print parsing \
                 error but this may leak secrets.",
                path.as_ref().display()
            )),
        }
    }

    pub async fn to_path<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        Ok(tokio::fs::write(path, toml::to_string_pretty(self)?).await?)
    }

    // Note for reviewers: if this and other validations are always applied,
    // we should instead move them to the deserialization stage
    // https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
    pub fn validate(self) -> anyhow::Result<Self> {
        ensure!(
            !self.drivers.is_empty(),
            "colocation is enabled but no drivers are configured"
        );
        Ok(self)
    }
}

#[cfg(any(test, feature = "test-util"))]
impl Configuration {
    pub fn to_temp_path(&self) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().expect("temp file creation should not fail");
        file.write_all(
            toml::to_string_pretty(self)
                .expect("serialization should not fail")
                .as_bytes(),
        )
        .expect("writing to temp file should not fail");
        file
    }

    pub fn to_cli_args(&self) -> (tempfile::NamedTempFile, String) {
        // Must return the temp_file because it gets deleted on drop
        // disabling the cleanup will lead to a bunch of artifacts laying around
        let named_temp_file = self.to_temp_path();
        let cli_arg = format!("--config={}", named_temp_file.path().display());
        (named_temp_file, cli_arg)
    }
}
