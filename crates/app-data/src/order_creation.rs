//! The `appData` and `appDataHash` fields of order creation and quote
//! requests.

use {
    crate::{AppDataHash, hash_full_app_data},
    serde::{Deserialize, Deserializer, Serialize, de},
};

/// The order's AppData (can be an hash, the JSON body or both).
// Note that the order of the variants is important for deserialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum OrderCreationAppData {
    /// Hash is inferred from full app data and validated against expectation.
    Both {
        #[serde(rename = "appData")]
        full: String,
        #[serde(rename = "appDataHash")]
        expected: AppDataHash,
    },
    /// Backward compatible app data hash.
    Hash {
        #[serde(rename = "appData")]
        hash: AppDataHash,
    },
    /// Hash is inferred from full app data.
    Full {
        #[serde(rename = "appData")]
        full: String,
    },
}

impl Default for OrderCreationAppData {
    fn default() -> Self {
        Self::Hash {
            hash: Default::default(),
        }
    }
}

impl From<AppDataHash> for OrderCreationAppData {
    fn from(hash: AppDataHash) -> Self {
        Self::Hash { hash }
    }
}

impl OrderCreationAppData {
    /// Returns the app data hash. Does not validate expectation.
    pub fn hash(&self) -> AppDataHash {
        match self {
            Self::Hash { hash } => *hash,
            Self::Full { full } | Self::Both { full, .. } => {
                AppDataHash(hash_full_app_data(full.as_bytes()))
            }
        }
    }

    /// The form the `appData` and `appDataHash` fields name, `None` when both
    /// are absent. `appData` is a hash when it parses as one, else a document.
    pub fn from_fields(
        app_data: Option<String>,
        app_data_hash: Option<AppDataHash>,
    ) -> Result<Option<Self>, HashWithoutDocument> {
        Ok(Some(match (app_data, app_data_hash) {
            (Some(app_data), None) => match app_data.parse() {
                Ok(hash) => Self::Hash { hash },
                Err(_) => Self::Full { full: app_data },
            },
            (Some(full), Some(expected)) => Self::Both { full, expected },
            (None, None) => return Ok(None),
            (None, Some(_)) => return Err(HashWithoutDocument),
        }))
    }
}

/// `appDataHash` without the `appData` document it is the hash of.
#[derive(Debug, thiserror::Error)]
#[error("invalid app data")]
pub struct HashWithoutDocument;

/// Deserializes the `appData` and `appDataHash` fields, the zero hash when
/// both are absent. `#[serde(default, flatten)]` does not work as expected
/// and generates errors for requests without `appData`.
pub fn deserialize_optional_app_data<'de, D>(
    deserializer: D,
) -> Result<OrderCreationAppData, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Helper {
        app_data: Option<String>,
        app_data_hash: Option<AppDataHash>,
    }

    let Helper {
        app_data,
        app_data_hash,
    } = Helper::deserialize(deserializer)?;
    OrderCreationAppData::from_fields(app_data, app_data_hash)
        .map(Option::unwrap_or_default)
        .map_err(de::Error::custom)
}
