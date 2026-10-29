use {
    crate::{
        database::{Postgres, app_data::InsertError},
        ipfs_app_data::IpfsAppData,
    },
    anyhow::{Context, Result},
    app_data::AppDataHash,
    moka::sync::Cache,
};

/// Approximate heap bytes a cache entry takes besides its document: the key
/// and moka's bookkeeping, measured with moka 0.12.
const CACHE_ENTRY_OVERHEAD: usize = 280;

/// CoW Protocol API app-data registry.
pub struct Registry {
    validator: app_data::Validator,
    database: Postgres,
    ipfs: Option<IpfsAppData>,
    /// Full app data found in the database. A hash always maps to the same
    /// document, so entries never go stale.
    cache: Cache<AppDataHash, String>,
}

impl Registry {
    /// Creates a new instance of an app-data registry.
    pub fn new(
        validator: app_data::Validator,
        database: Postgres,
        ipfs: Option<IpfsAppData>,
        cache_size: u64,
    ) -> Self {
        Self {
            validator,
            database,
            ipfs,
            // Each entry weighs its approximate heap size in bytes, and moka
            // keeps the total within `cache_size`. A full cache admits a new
            // document only if it is read more often than the least recently
            // used entries it would evict (TinyLFU). An entry heavier than
            // `cache_size` is never cached.
            cache: Cache::builder()
                .max_capacity(cache_size)
                .weigher(|_, document: &String| {
                    u32::try_from(CACHE_ENTRY_OVERHEAD + document.len()).unwrap_or(u32::MAX)
                })
                .build(),
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
    ) -> Result<Register, RegisterError> {
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
            Ok(()) => Ok(Register {
                status: RegistrationStatus::New,
                hash: validated.hash,
            }),
            Err(InsertError::Duplicate) => Ok(Register {
                status: RegistrationStatus::AlreadyExisted,
                hash: validated.hash,
            }),
            Err(InsertError::Mismatch(existing)) => Err(RegisterError::DataMismatch { existing }),
            Err(InsertError::Other(err)) => Err(RegisterError::Other(err)),
        }
    }

    /// Returns the full app data stored in the database for the given hash.
    pub async fn get_full_app_data(
        &self,
        contract_app_data: &AppDataHash,
    ) -> Result<Option<String>> {
        if let Some(document) = self.cache.get(contract_app_data) {
            return Ok(Some(document));
        }
        let document = self.database.get_full_app_data(contract_app_data).await?;
        // A missing document can still be registered later, so only hits are
        // cached.
        if let Some(document) = &document {
            self.cache.insert(*contract_app_data, document.clone());
        }
        Ok(document)
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
pub enum RegistrationStatus {
    /// The app data was newly added to the registry.
    New,
    /// An identical app data was already registered.
    AlreadyExisted,
}

pub struct Register {
    pub status: RegistrationStatus,
    pub hash: AppDataHash,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_caches_found_app_data_only() {
        let db = Postgres::try_new("postgresql://", Default::default()).unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();
        let registry = Registry::new(app_data::Validator::new(8192), db.clone(), None, 1024);
        let document = r#"{"appCode":"cache"}"#;
        let hash = registry
            .validator
            .validate(document.as_bytes())
            .unwrap()
            .hash;

        assert_eq!(registry.get_full_app_data(&hash).await.unwrap(), None);
        registry.register(None, document.as_bytes()).await.unwrap();
        assert_eq!(
            registry.get_full_app_data(&hash).await.unwrap().as_deref(),
            Some(document)
        );

        database::clear_DANGER(&db.pool).await.unwrap();
        assert_eq!(
            registry.get_full_app_data(&hash).await.unwrap().as_deref(),
            Some(document)
        );
    }
}
