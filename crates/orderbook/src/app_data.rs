use {
    crate::database::{app_data::InsertError, Postgres},
    model::app_id::AppDataHash,
    shared::app_data,
};

/// CoW Protocol API app-data registry.
pub struct Registry {
    validator: app_data::Validator,
    database: Postgres,
}

impl Registry {
    /// Creates a new instance of an app-data registry.
    pub fn new(validator: app_data::Validator, database: Postgres) -> Self {
        Self {
            validator,
            database,
        }
    }

    /// Returns the size limit, in bytes, of an app-data document.
    pub fn size_limit(&self) -> usize {
        self.validator.size_limit()
    }

    /// Saves an app-data document matching the specified app-data hash to the
    /// database.
    pub async fn save(&self, hash: AppDataHash, document: &[u8]) -> Result<(), SaveError> {
        let validated = self
            .validator
            .validate(document)
            .map_err(SaveError::Invalid)?;
        if hash != validated.hash {
            return Err(SaveError::HashMismatch {
                expected: hash,
                computed: validated.hash,
            });
        }

        self.database
            .insert_full_app_data(&validated.hash, &validated.document)
            .await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("app data is invalid: {0}")]
    Invalid(anyhow::Error),
    #[error("app data already exists")]
    Duplicate,
    #[error("computed app data hash {computed:?} doesn't match expected {expected:?}")]
    HashMismatch {
        expected: AppDataHash,
        computed: AppDataHash,
    },
    #[error("stored app data {existing:?} is different than the specified data")]
    DataMismatch { existing: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<InsertError> for SaveError {
    fn from(value: InsertError) -> Self {
        match value {
            InsertError::Duplicate => SaveError::Duplicate,
            InsertError::Mismatch(existing) => SaveError::DataMismatch { existing },
            InsertError::Other(err) => SaveError::Other(err),
        }
    }
}
