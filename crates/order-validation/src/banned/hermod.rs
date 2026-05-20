//! Hermod (zeroShadow) sanctioned-address checker.
//!
//! Queries are HMAC-SHA256-signed using a per-customer key; a hit returns
//! HTTP 200 and a miss returns HTTP 404. Mirrors the structure of the
//! Chainalysis `Onchain` checker: same cache, same background refresh task.

use {
    super::{Backend, UserMetadata},
    alloy_primitives::Address,
    futures::future::join_all,
    hmac::{Hmac, Mac},
    moka::sync::Cache,
    sha2::Sha256,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    url::Url,
};

const CACHE_EXPIRY: Duration = Duration::from_secs(60 * 60);
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

/// Configuration for the Hermod (zeroShadow) sanctioned-address checker.
#[derive(Debug, Clone)]
pub struct HermodConfig {
    /// Base URL of the Hermod agent (e.g. `http://hermod:3000`).
    pub url: Url,
    /// Per-customer HMAC key used to obfuscate addresses before sending.
    pub hmac_key: String,
    /// Optional API key sent as a Bearer token.
    pub api_key: Option<String>,
}

// Fields are read only through the `Debug` derive used in error logs, which
// rustc's dead-code analysis ignores — suppress the false-positive.
#[allow(dead_code)]
#[derive(Debug)]
pub(super) enum FetchError {
    Request(reqwest::Error),
    UnexpectedStatus(reqwest::StatusCode),
}

impl From<reqwest::Error> for FetchError {
    fn from(err: reqwest::Error) -> Self {
        Self::Request(err)
    }
}

/// Hermod banned user checker with caching and background refresh.
pub(super) struct Hermod {
    client: reqwest::Client,
    url: Url,
    hmac_key: Vec<u8>,
    api_key: Option<String>,
    cache: Cache<Address, UserMetadata>,
}

impl Hermod {
    pub(super) fn new(config: HermodConfig, cache_max_size: u64) -> Arc<Self> {
        // Make sure the URL ends with a slash so joining `addresses/<sig>`
        // appends rather than replaces the last path segment.
        let mut url = config.url;
        if !url.path().ends_with('/') {
            let with_slash = format!("{}/", url.path());
            url.set_path(&with_slash);
        }
        let hermod = Arc::new(Self {
            client: reqwest::Client::new(),
            url,
            hmac_key: config.hmac_key.into_bytes(),
            api_key: config.api_key,
            cache: Cache::builder().max_capacity(cache_max_size).build(),
        });

        hermod.clone().spawn_maintenance_task();

        hermod
    }

    /// Returns the cache entries whose age is close enough to expiry that
    /// they should be refreshed during this maintenance cycle. Computed
    /// relative to `start` (rather than `Instant::now()`) so the threshold
    /// is consistent across one iteration of the maintenance loop.
    fn expired_data(&self, start: Instant) -> Vec<(Arc<Address>, UserMetadata)> {
        self.cache
            .iter()
            .filter_map(|(address, metadata)| {
                let expired = start
                    .checked_duration_since(metadata.last_updated)
                    .unwrap_or_default()
                    >= CACHE_EXPIRY - MAINTENANCE_TIMEOUT;

                expired.then_some((address, metadata))
            })
            .collect()
    }

    /// Re-fetches the ban status for a single expired cache entry during
    /// background maintenance, returning the refreshed `(address, metadata)`
    /// pair on success. Fetch errors are logged and yield `None` so the
    /// existing cache entry is left in place rather than being overwritten
    /// with stale or wrong data.
    async fn determine_status(
        &self,
        address: Address,
        metadata: UserMetadata,
    ) -> Option<(Address, UserMetadata)> {
        self.fetch(address)
            .await
            .inspect_err(|err| {
                tracing::warn!(
                    address = ?*address,
                    ?err,
                    "unable to determine hermod banned status in the background task"
                )
            })
            .ok()
            .map(|is_banned| {
                (
                    address,
                    UserMetadata {
                        is_banned,
                        ..metadata
                    },
                )
            })
    }

    fn spawn_maintenance_task(self: Arc<Self>) {
        let detector = Arc::clone(&self);

        tokio::task::spawn(async move {
            loop {
                let start = Instant::now();
                let expired_data: Vec<_> = detector.expired_data(start);

                let results = join_all(
                    expired_data
                        .into_iter()
                        .map(|(address, metadata)| detector.determine_status(*address, metadata)),
                )
                .await
                .into_iter()
                .flatten();
                detector.insert_many_into_cache(results);

                let remaining_sleep = MAINTENANCE_TIMEOUT
                    .checked_sub(start.elapsed())
                    .unwrap_or_default();
                tokio::time::sleep(remaining_sleep).await;
            }
        });
    }

    fn insert_many_into_cache(&self, addresses: impl Iterator<Item = (Address, UserMetadata)>) {
        let now = Instant::now();
        for (address, metadata) in addresses {
            self.cache.insert(
                address,
                UserMetadata {
                    last_updated: now,
                    ..metadata
                },
            );
        }
    }

    /// HMAC-SHA256 of the address textual payload, encoded as lowercase hex.
    /// The payload is the lowercase `0x`-prefixed 40-character form, matching
    /// the EVM test address shown in the Hermod docs and what the agent is
    /// configured against on our side. The exact textual form must agree with
    /// the agent or every signature will be a miss.
    fn sign(&self, address: Address) -> String {
        let payload = format!("{address:#x}");
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.hmac_key)
            .expect("HMAC accepts keys of any length");
        mac.update(payload.as_bytes());
        const_hex::encode(mac.finalize().into_bytes())
    }
}

impl Backend for Hermod {
    type Error = FetchError;

    async fn fetch(&self, address: Address) -> Result<bool, Self::Error> {
        let signature = self.sign(address);
        let endpoint = self
            .url
            .join(&format!("addresses/{signature}"))
            .expect("base url is valid and signature is hex");

        let mut request = self.client.get(endpoint);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await?;
        match response.status() {
            reqwest::StatusCode::OK => Ok(true),
            reqwest::StatusCode::NOT_FOUND => Ok(false),
            status => Err(FetchError::UnexpectedStatus(status)),
        }
    }

    fn cache(&self) -> &Cache<Address, UserMetadata> {
        &self.cache
    }

    fn name(&self) -> &'static str {
        "hermod"
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address};

    fn backend() -> Arc<Hermod> {
        Hermod::new(
            HermodConfig {
                url: "http://hermod:3000".parse().unwrap(),
                hmac_key: "key".to_string(),
                api_key: None,
            },
            10,
        )
    }

    #[tokio::test]
    async fn hmac_signature_is_deterministic() {
        let hermod = backend();
        let addr = address!("dead000000000000000000000000000000000000");
        assert_eq!(hermod.sign(addr), hermod.sign(addr));
        assert_eq!(hermod.sign(addr).len(), 64);
    }

    #[tokio::test]
    async fn base_url_without_trailing_slash_is_normalised() {
        let hermod = Hermod::new(
            HermodConfig {
                url: "http://hermod:3000/v1".parse().unwrap(),
                hmac_key: "key".to_string(),
                api_key: None,
            },
            10,
        );
        assert!(hermod.url.as_str().ends_with('/'));
        let joined = hermod.url.join("addresses/abc").unwrap();
        assert_eq!(joined.as_str(), "http://hermod:3000/v1/addresses/abc");
    }
}
