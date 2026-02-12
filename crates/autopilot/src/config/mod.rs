use {
    crate::config::solver::Solver,
    anyhow::ensure,
    serde::{Deserialize, Serialize},
    std::path::Path,
};

pub mod solver;

#[derive(Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub drivers: Vec<Solver>,
}

impl Configuration {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Ok(toml::from_str(&tokio::fs::read_to_string(path).await?)?)
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

impl Default for Configuration {
    fn default() -> Self {
        Self {
            drivers: Default::default(),
        }
    }
}
