use {
    crate::database::{app_data::InsertError, Postgres},
    model::app_data::AppDataHash,
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

    /// Registers an app-data document matching the specified app-data hash to
    /// the registry, ensuring that there exists an entry linking the specified
    /// app data hash with the document.
    ///
    /// Returns `New` if the app data was newly added or `AlreadyExisted` if an
    /// exactly matching entry already existed.
    pub async fn register(
        &self,
        hash: AppDataHash,
        document: &[u8],
    ) -> Result<Registered, RegisterError> {
        let validated = self
            .validator
            .validate(document)
            .map_err(RegisterError::Invalid)?;
        if hash != validated.hash {
            return Err(RegisterError::HashMismatch {
                expected: hash,
                computed: validated.hash,
            });
        }

        match self
            .database
            .insert_full_app_data(&validated.hash, &validated.document)
            .await
        {
            Ok(()) => Ok(Registered::New),
            Err(InsertError::Duplicate) => Ok(Registered::AlreadyExisted),
            Err(InsertError::Mismatch(existing)) => Err(RegisterError::DataMismatch { existing }),
            Err(InsertError::Other(err)) => Err(RegisterError::Other(err)),
        }
    }
}

#[derive(Debug)]
pub enum Registered {
    /// The app data was newly added to the registry.
    New,
    /// An identical app data was already registered.
    AlreadyExisted,
}

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("appData is invalid: {0}")]
    Invalid(anyhow::Error),
    #[error("computed appDataHash {computed:?} doesn't match expected {expected:?}")]
    HashMismatch {
        expected: AppDataHash,
        computed: AppDataHash,
    },
    #[error("stored appData {existing:?} is different than the specified data")]
    DataMismatch { existing: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
