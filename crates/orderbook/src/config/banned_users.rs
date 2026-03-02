use {
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::num::NonZeroUsize,
};

const fn default_max_cache_size() -> NonZeroUsize {
    NonZeroUsize::new(100).expect("value should be non-zero")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BannedUsersConfig {
    #[serde(default)]
    pub addresses: Vec<Address>,

    /// Maximum number of entries to keep in the banned users cache.
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: NonZeroUsize,
}

impl Default for BannedUsersConfig {
    fn default() -> Self {
        Self {
            addresses: Default::default(),
            max_cache_size: default_max_cache_size(),
        }
    }
}
