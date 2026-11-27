//! Hermod (zeroShadow) sanctioned-address client. Addresses are sent as
//! HMAC-SHA256 signatures, at most [`MAX_BATCH_SIZE`] per
//! `GET /address_batches` request.

use {
    super::{Address, metrics::Metrics},
    futures::future::join_all,
    hmac::{Hmac, Mac},
    moka::{policy::EvictionPolicy, sync::Cache},
    serde::Deserialize,
    sha2::Sha256,
    std::{
        collections::{HashMap, HashSet},
        time::{Duration, Instant},
    },
    url::Url,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
/// Hermod rejects batches with more unique signatures than this.
const MAX_BATCH_SIZE: usize = 1000;

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

/// The HMAC-SHA256 signature of an address.
type Signature = [u8; 32];

/// One known threat in a `GET /address_batches` response.
#[derive(Deserialize)]
struct Hit {
    #[serde(deserialize_with = "const_hex::deserialize")]
    hashed_address: Signature,
}

/// What Hermod reports for an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    Banned,
    NotBanned,
    /// The request failed, so Hermod gave no verdict.
    Unknown,
}

pub(super) struct Client<A: Address> {
    client: reqwest::Client,
    endpoint: Url,
    hmac_key: Vec<u8>,
    api_key: Option<String>,
    /// HMAC signature of each recently checked address, so repeated checks
    /// do not compute it again.
    signatures: Cache<A, Signature>,
}

impl<A: Address> Client<A> {
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
        }
    }

    /// HMAC-SHA256 of the address bytes as lowercase `0x`-prefixed hex (42
    /// chars for an EVM address).
    fn sign(&self, address: A) -> Signature {
        self.signatures.get_with(address, || {
            let mut mac = Hmac::<Sha256>::new_from_slice(&self.hmac_key)
                .expect("HMAC accepts keys of any length");
            mac.update(const_hex::encode_prefixed(address).as_bytes());
            mac.finalize().into_bytes().into()
        })
    }

    /// Returns the status of each address. The addresses of a failed batch
    /// request are [`Status::Unknown`].
    pub(super) async fn fetch_statuses(&self, addresses: &HashSet<A>) -> HashMap<A, Status> {
        let signed: Vec<(Signature, A)> = addresses
            .iter()
            .map(|address| (self.sign(*address), *address))
            .collect();
        join_all(
            signed
                .chunks(MAX_BATCH_SIZE)
                .map(|batch| self.fetch_batch_statuses(batch)),
        )
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    /// Checks `batch` with one request. Requires `batch` to have at most
    /// [`MAX_BATCH_SIZE`] items; extra items are not checked.
    async fn fetch_batch_statuses(&self, batch: &[(Signature, A)]) -> Vec<(A, Status)> {
        let batch = &batch[..batch.len().min(MAX_BATCH_SIZE)];
        let start = Instant::now();
        let result = self.request_batch(batch).await;
        Metrics::batch(
            batch.len(),
            result.as_ref().ok().map(Vec::len),
            start.elapsed(),
        );
        if let Err(err) = &result {
            tracing::warn!(?err, size = batch.len(), "failed to fetch banned status");
        }
        statuses_of_batch(batch, result.as_deref().ok())
    }

    async fn request_batch(&self, batch: &[(Signature, A)]) -> Result<Vec<Hit>, Error> {
        // Stack buffers: no heap allocation per signature.
        let hex: Vec<_> = batch
            .iter()
            .map(|(signature, _)| const_hex::Buffer::<32>::new().const_format(signature))
            .collect();
        let body: Vec<&str> = hex.iter().map(const_hex::Buffer::as_str).collect();
        let mut request = self.client.get(self.endpoint.clone()).json(&body);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request
            .send()
            .await
            .inspect_err(|_| Metrics::response_status("no_response"))?;
        Metrics::response_status(response.status().as_str());
        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json().await?),
            reqwest::StatusCode::NOT_FOUND => Ok(Vec::new()),
            status => Err(Error::UnexpectedStatus(status)),
        }
    }
}

/// Returns the status of each address in `batch`. `hits` are the threats that
/// Hermod reported for the batch, or `None` if the request failed.
fn statuses_of_batch<A: Address>(
    batch: &[(Signature, A)],
    hits: Option<&[Hit]>,
) -> Vec<(A, Status)> {
    batch
        .iter()
        .map(|(signature, address)| {
            let status = match hits {
                // Linear scan: hits are rare (~10 per mainnet update).
                Some(hits) if hits.iter().any(|hit| hit.hashed_address == *signature) => {
                    Status::Banned
                }
                Some(_) => Status::NotBanned,
                None => Status::Unknown,
            };
            (*address, status)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    fn client(url: &str) -> Client<[u8; 20]> {
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
            hermod.sign(hex!("dead000000000000000000000000000000000000")),
            hex!("e146f8bc38fe78f9356be67706a02eb3ce386b4b12a14ab1f76d022020b6cef0"),
        );
    }

    #[test]
    fn hit_reads_the_signature_as_hex() {
        let signature = hex!("e146f8bc38fe78f9356be67706a02eb3ce386b4b12a14ab1f76d022020b6cef0");
        for prefix in ["", "0x"] {
            let json = format!(
                r#"{{"hashed_address": "{prefix}{}"}}"#,
                const_hex::encode(signature)
            );
            let hit: Hit = serde_json::from_str(&json).unwrap();
            assert_eq!(hit.hashed_address, signature);
        }
    }

    #[test]
    fn batch_statuses_follow_the_response() {
        let (safe, threat) = ([1u8; 20], [2u8; 20]);
        let batch = [([1; 32], safe), ([2; 32], threat)];
        let hits = [Hit {
            hashed_address: [2; 32],
        }];

        assert_eq!(
            statuses_of_batch(&batch, Some(&hits)),
            [(safe, Status::NotBanned), (threat, Status::Banned)],
        );
        assert_eq!(
            statuses_of_batch(&batch, None),
            [(safe, Status::Unknown), (threat, Status::Unknown)],
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
