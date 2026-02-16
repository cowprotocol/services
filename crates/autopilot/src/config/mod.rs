use {
    crate::config::solver::Solver,
    anyhow::{anyhow, ensure},
    serde::{Deserialize, Serialize},
    std::path::Path,
};

pub mod solver;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    #[serde(default)]
    pub drivers: Vec<Solver>,
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

    #[cfg(any(test, feature = "test-util"))]
    pub fn to_temp_path(&self) -> anyhow::Result<tempfile::NamedTempFile> {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        Ok(file)
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
