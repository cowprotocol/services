use {
    anyhow::{anyhow, Context, Result},
    model::{app_id::AppDataHash, order::Interactions},
    serde::Deserialize,
    serde_json::Value,
};

#[derive(Debug)]
pub struct ValidatedAppData {
    pub hash: AppDataHash,
    pub backend: BackendAppData,
}

#[derive(Debug, Default, Deserialize)]
pub struct BackendAppData {
    #[serde(default)]
    pub interactions: Interactions,
}

#[derive(Clone)]
pub struct Validator {
    size_limit: usize,
}

impl Default for Validator {
    fn default() -> Self {
        Self { size_limit: 8192 }
    }
}

impl Validator {
    pub fn validate(&self, full_app_data: &[u8]) -> Result<ValidatedAppData> {
        if full_app_data.len() > self.size_limit {
            return Err(anyhow!(
                "app data has byte size {} which is larger than limit {}",
                full_app_data.len(),
                self.size_limit
            ));
        }

        let mut json: Value = serde_json::from_slice(full_app_data).context("invalid json")?;
        let json = json.as_object_mut().context("top level isn't object")?;
        let backend: BackendAppData = json
            .remove("backend")
            .map(serde_json::from_value)
            .transpose()
            .context("top level `backend` value doesn't match schema")?
            // If the key doesn't exist, default. Makes life easier for API consumers, who don't care about backend app data.
            .unwrap_or_default();

        // Perform potentially more backend app data validation here.

        Ok(ValidatedAppData {
            hash: AppDataHash(app_data_hash::hash_full_app_data(full_app_data)),
            backend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {
        let mut validator = Validator { size_limit: 100 };

        let not_json = "hello world".as_bytes();
        let err = validator.validate(not_json).unwrap_err();
        dbg!(err);

        let not_object = "[]".as_bytes();
        let err = validator.validate(not_object).unwrap_err();
        dbg!(err);

        let object = "{}".as_bytes();
        let validated = validator.validate(object).unwrap();
        dbg!(validated.hash);

        let ok_no_backend = r#"{"hello":"world"}"#.as_bytes();
        validator.validate(ok_no_backend).unwrap();

        let bad_backend = r#"{"hello":"world","backend":[1]}"#.as_bytes();
        let err = validator.validate(bad_backend).unwrap_err();
        dbg!(err);

        let ok_backend = r#"{"hello":"world","backend":{}}"#.as_bytes();
        validator.validate(ok_backend).unwrap();

        validator.size_limit = 1;
        let size_limit = r#"{"hello":"world"}"#.as_bytes();
        let err = validator.validate(size_limit).unwrap_err();
        dbg!(err);
    }
}
