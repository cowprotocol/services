use {
    crate::{
        database::{app_data::InsertError, Postgres},
        ipfs_app_data::IpfsAppData,
    },
    anyhow::{Context, Result},
    model::app_data::AppDataHash,
};

/// CoW Protocol API app-data registry.
pub struct Registry {
    validator: app_data::Validator,
    database: Postgres,
    ipfs: Option<IpfsAppData>,
}

impl Registry {
    /// Creates a new instance of an app-data registry.
    pub fn new(
        validator: app_data::Validator,
        database: Postgres,
        ipfs: Option<IpfsAppData>,
    ) -> Self {
        Self {
            validator,
            database,
            ipfs,
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
        hash: Option<AppDataHash>,
        document: &[u8],
    ) -> Result<(Registered, AppDataHash), RegisterError> {
        let validated = self
            .validator
            .validate(document)
            .map_err(RegisterError::Invalid)?;
        if hash.is_some_and(|hash| hash != validated.hash) {
            return Err(RegisterError::HashMismatch {
                expected: hash.unwrap(),
                computed: validated.hash,
            });
        }

        match self
            .database
            .insert_full_app_data(&validated.hash, &validated.document)
            .await
        {
            Ok(()) => Ok((Registered::New, validated.hash)),
            Err(InsertError::Duplicate) => Ok((Registered::AlreadyExisted, validated.hash)),
            Err(InsertError::Mismatch(existing)) => Err(RegisterError::DataMismatch { existing }),
            Err(InsertError::Other(err)) => Err(RegisterError::Other(err)),
        }
    }

    /// Finds full app data for an order that only has the contract app data
    /// hash.
    ///
    /// The full app data can be located in the database or on IPFS.
    pub async fn find(&self, contract_app_data: &AppDataHash) -> Result<Option<String>> {
        // we reserve the 0 app data to indicate empty app data.
        if contract_app_data.is_zero() {
            return Ok(Some(app_data::EMPTY.to_string()));
        }

        if let Some(app_data) = self
            .database
            .get_full_app_data(contract_app_data)
            .await
            .context("from database")?
        {
            tracing::debug!(?contract_app_data, "full app data in database");
            return Ok(Some(app_data));
        }

        let Some(ipfs) = &self.ipfs else {
            return Ok(None);
        };
        ipfs.fetch(contract_app_data).await.context("from ipfs")
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
