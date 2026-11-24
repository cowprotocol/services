//! Hermod (zeroShadow) sanctioned-address client. Addresses are sent as
//! HMAC-SHA256 signatures, at most [`MAX_BATCH_SIZE`] per
//! `GET /address_batches` request.

use {
    super::metrics::Metrics,
    alloy_primitives::{Address, B256},
    futures::future::join_all,
    hmac::{Hmac, Mac},
    moka::{policy::EvictionPolicy, sync::Cache},
    serde::Deserialize,
    sha2::Sha256,
    std::{
        collections::HashSet,
        time::{Duration, Instant},
    },
    url::Url,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
/// Hermod rejects batches with more unique signatures than this.
const MAX_BATCH_SIZE: usize = 1000;
/// How long a reported threat stays banned while Hermod cannot be reached.
const BANNED_FALLBACK_TTL: Duration = Duration::from_secs(10 * 60);
const BANNED_FALLBACK_CAPACITY: u64 = 1000;

/// Configuration for the Hermod (zeroShadow) sanctioned-address checker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Base URL of the Hermod agent (e.g. `http://hermod:3000`).
    pub url: Url,
    /// Per-customer HMAC key used to obfuscate addresses before sending.
    pub hmac_key: String,
    /// Optional API key sent as a Bearer token.
    pub api_key: Option<String>,
    /// How often the cached addresses are checked again.
    pub refresh_interval: Duration,
    /// An address that nobody requested for this long leaves the cache.
    pub max_idle: Duration,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("request failed")]
    Request(#[from] reqwest::Error),
    #[error("unexpected status code: {0}")]
    UnexpectedStatus(reqwest::StatusCode),
}

/// One known threat in a `GET /address_batches` response.
#[derive(Deserialize)]
struct Hit {
    hashed_address: B256,
}

pub(super) struct Client {
    client: reqwest::Client,
    endpoint: Url,
    hmac_key: Vec<u8>,
    api_key: Option<String>,
    /// HMAC signature of each recently checked address, so repeated checks
    /// do not compute it again.
    signatures: Cache<Address, B256>,
    /// Recent threats, used only when a batch request fails.
    recently_banned: Cache<Address, ()>,
}

impl Client {
    pub(super) fn new(config: Config, max_cached_signatures: u64) -> Self {
        // Make sure the URL ends with a slash so joining `address_batches`
        // appends rather than replaces the last path segment.
        let mut url = config.url;
        if !url.path().ends_with('/') {
            let with_slash = format!("{}/", url.path());
            url.set_path(&with_slash);
        }
        Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client builder with default TLS settings is infallible"),
            endpoint: url
                .join("address_batches")
                .expect("base url is valid and the path is static"),
            hmac_key: config.hmac_key.into_bytes(),
            api_key: config.api_key,
            signatures: Cache::builder()
                .max_capacity(max_cached_signatures)
                .eviction_policy(EvictionPolicy::lru())
                .build(),
            recently_banned: Cache::builder()
                .max_capacity(BANNED_FALLBACK_CAPACITY)
                .time_to_live(BANNED_FALLBACK_TTL)
                .build(),
        }
    }

    /// HMAC-SHA256 of the lowercase `0x`-prefixed address (42 chars).
    fn sign(&self, address: Address) -> B256 {
        self.signatures.get_with(address, || {
            let mut mac = Hmac::<Sha256>::new_from_slice(&self.hmac_key)
                .expect("HMAC accepts keys of any length");
            mac.update(const_hex::encode_prefixed(address).as_bytes());
            B256::from(<[u8; 32]>::from(mac.finalize().into_bytes()))
        })
    }

    /// Returns the subset of `addresses` that Hermod reports as threats.
    /// If a batch request fails, its addresses that Hermod reported in the
    /// last [`BANNED_FALLBACK_TTL`] stay banned; the others pass.
    pub(super) async fn banned(&self, addresses: &HashSet<Address>) -> HashSet<Address> {
        let signed: Vec<(B256, Address)> = addresses
            .iter()
            .map(|address| (self.sign(*address), *address))
            .collect();
        join_all(
            signed
                .chunks(MAX_BATCH_SIZE)
                .map(|batch| self.banned_in_batch(batch)),
        )
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    /// Checks `batch` with one request. Requires `batch` to have at most
    /// [`MAX_BATCH_SIZE`] items; extra items are not checked.
    async fn banned_in_batch(&self, batch: &[(B256, Address)]) -> Vec<Address> {
        let batch = &batch[..batch.len().min(MAX_BATCH_SIZE)];
        let start = Instant::now();
        let result = self.request_batch(batch).await;
        Metrics::batch(
            batch.len(),
            result.as_ref().ok().map(Vec::len),
            start.elapsed(),
        );

        match result {
            Ok(hits) => self.map_hits_to_addresses(hits, batch),
            Err(err) => self.fall_back_to_local_cache(batch, err),
        }
    }

    /// Maps the reported signatures back to their addresses and records them
    /// in the local cache.
    fn map_hits_to_addresses(&self, hits: Vec<Hit>, batch: &[(B256, Address)]) -> Vec<Address> {
        hits.into_iter()
            .filter_map(|Hit { hashed_address }| {
                // Linear scan: hits are rare (~10 per mainnet update), and
                // below ~50 hits per 1000 addresses it beats a HashMap.
                let Some((_, address)) = batch
                    .iter()
                    .find(|(signature, _)| *signature == hashed_address)
                else {
                    tracing::warn!(%hashed_address, "hermod returned an unrequested signature");
                    return None;
                };
                self.recently_banned.insert(*address, ());
                Some(*address)
            })
            .collect()
    }

    /// Addresses in `batch` that Hermod reported in the last
    /// [`BANNED_FALLBACK_TTL`].
    fn fall_back_to_local_cache(&self, batch: &[(B256, Address)], err: Error) -> Vec<Address> {
        let banned: Vec<Address> = batch
            .iter()
            .map(|(_, address)| *address)
            .filter(|address| self.recently_banned.contains_key(address))
            .collect();
        tracing::warn!(
            ?err,
            size = batch.len(),
            recently_banned = banned.len(),
            "failed to fetch banned status, falling back to local cache",
        );
        banned
    }

    async fn request_batch(&self, batch: &[(B256, Address)]) -> Result<Vec<Hit>, Error> {
        // Stack buffers: no heap allocation per signature.
        let hex: Vec<_> = batch
            .iter()
            .map(|(signature, _)| const_hex::Buffer::<32>::new().const_format(&signature.0))
            .collect();
        let body: Vec<&str> = hex.iter().map(const_hex::Buffer::as_str).collect();
        let mut request = self.client.get(self.endpoint.clone()).json(&body);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await?;
        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json().await?),
            reqwest::StatusCode::NOT_FOUND => Ok(Vec::new()),
            status => Err(Error::UnexpectedStatus(status)),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{address, b256},
    };

    fn client(url: &str) -> Client {
        Client::new(
            Config {
                url: url.parse().unwrap(),
                hmac_key: "key".to_string(),
                api_key: None,
                refresh_interval: Duration::from_secs(5),
                max_idle: Duration::from_secs(60),
            },
            100,
        )
    }

    #[test]
    fn signature_matches_reference() {
        // printf '%s' 0xdead000000000000000000000000000000000000 \
        //   | openssl dgst -sha256 -hmac key
        let hermod = client("http://hermod:3000");
        assert_eq!(
            hermod.sign(address!("dead000000000000000000000000000000000000")),
            b256!("e146f8bc38fe78f9356be67706a02eb3ce386b4b12a14ab1f76d022020b6cef0"),
        );
    }

    #[test]
    fn base_url_without_trailing_slash_is_normalised() {
        let hermod = client("http://hermod:3000/v1");
        assert_eq!(
            hermod.endpoint.as_str(),
            "http://hermod:3000/v1/address_batches"
        );
    }
}
