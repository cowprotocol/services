//! Native token prices from CoinGecko, denominated in wSOL.

use {
    crate::infra::config,
    anyhow::{Context, Result, anyhow},
    chain_types::{ChainTypes, solana::Solana},
    cow_solana_rpc::SolanaRPC,
    serde::Deserialize,
    solana_sdk::{program_pack::Pack, pubkey::Pubkey},
    spl_token_interface::state::Mint,
    std::{
        collections::{HashMap, HashSet},
        sync::Mutex,
        time::{Duration, Instant},
    },
    url::Url,
};

/// CoinGecko's authorization header.
const API_KEY_HEADER: &str = "x-cg-pro-api-key";

/// Native price lookups for auction tokens, cached per token.
pub struct NativePrices {
    client: reqwest::Client,
    endpoint: Url,
    api_key: Option<String>,
    rpc: SolanaRPC,
    wrapped_native: Pubkey,
    ttl: Duration,
    /// Fetched prices by mint. `None` records a mint CoinGecko does not
    /// list, so unlisted mints are not refetched every cut.
    prices: Mutex<HashMap<Pubkey, (Instant, Option<u64>)>>,
    /// Mint decimals never change, so they are cached forever.
    decimals: Mutex<HashMap<Pubkey, u8>>,
}

/// One priced entry of the CoinGecko `simple/token_price` response.
#[derive(Debug, Deserialize)]
struct Entry {
    sol: Option<f64>,
}

impl NativePrices {
    pub fn new(config: &config::NativePrices, rpc: SolanaRPC, wrapped_native: Pubkey) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            rpc,
            wrapped_native,
            ttl: config.ttl,
            prices: Mutex::new(HashMap::new()),
            decimals: Mutex::new(HashMap::new()),
        }
    }

    /// A lookup pre-seeded for tests: the given prices never expire and
    /// nothing is fetched.
    #[cfg(test)]
    pub(crate) fn seeded(entries: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: "http://127.0.0.1:1/".parse().expect("literal url"),
            api_key: None,
            rpc: SolanaRPC::new_mock_with_mocks(Default::default()),
            wrapped_native: Pubkey::default(),
            ttl: Duration::from_secs(u64::MAX),
            prices: Mutex::new(
                entries
                    .into_iter()
                    .map(|(token, price)| (token, (Instant::now(), Some(price))))
                    .collect(),
            ),
            decimals: Mutex::new(HashMap::new()),
        }
    }

    /// The lamport value of one atom of each token, scaled by 10^9 like
    /// [`ChainTypes::NATIVE_PRICE_DENOMINATOR`]. Tokens CoinGecko does not
    /// list are absent from the result. Any lookup failure fails the whole
    /// call: a partially priced auction would rank solutions on incomparable
    /// scores.
    pub async fn prices(&self, tokens: HashSet<Pubkey>) -> Result<HashMap<Pubkey, u64>> {
        let mut result = HashMap::new();
        let mut fetch = Vec::new();
        let now = Instant::now();
        {
            let cache = self.prices.lock().expect("price cache poisoned");
            for token in tokens {
                if token == self.wrapped_native {
                    result.insert(token, Solana::NATIVE_PRICE_DENOMINATOR);
                    continue;
                }
                match cache.get(&token) {
                    Some((fetched, price)) if now.duration_since(*fetched) < self.ttl => {
                        if let Some(price) = price {
                            result.insert(token, *price);
                        }
                    }
                    _ => fetch.push(token),
                }
            }
        }
        if fetch.is_empty() {
            return Ok(result);
        }

        let decimals = self.decimals(&fetch).await?;
        let quoted = self.fetch(&fetch).await?;
        let mut cache = self.prices.lock().expect("price cache poisoned");
        for token in fetch {
            let price = quoted
                .get(&token.to_string())
                .and_then(|entry| entry.sol)
                .and_then(|sol| scale(sol, decimals[&token]));
            cache.insert(token, (now, price));
            if let Some(price) = price {
                result.insert(token, price);
            }
        }
        Ok(result)
    }

    /// Decimals per mint, from the cache or the mint accounts on chain.
    async fn decimals(&self, tokens: &[Pubkey]) -> Result<HashMap<Pubkey, u8>> {
        let mut result = HashMap::new();
        let mut fetch = Vec::new();
        {
            let cache = self.decimals.lock().expect("decimals cache poisoned");
            for token in tokens {
                match cache.get(token) {
                    Some(decimals) => {
                        result.insert(*token, *decimals);
                    }
                    None => fetch.push(*token),
                }
            }
        }
        if fetch.is_empty() {
            return Ok(result);
        }
        let accounts = self
            .rpc
            .multiple_accounts(fetch.iter().copied())
            .await
            .context("fetch mint accounts")?;
        let mut cache = self.decimals.lock().expect("decimals cache poisoned");
        for token in fetch {
            let account = accounts
                .get(&token)
                .ok_or_else(|| anyhow!("mint {token} does not exist"))?;
            let mint = Mint::unpack(&account.data)
                .with_context(|| format!("mint {token} does not unpack"))?;
            cache.insert(token, mint.decimals);
            result.insert(token, mint.decimals);
        }
        Ok(result)
    }

    /// One `simple/token_price` request for the given mints.
    async fn fetch(&self, tokens: &[Pubkey]) -> Result<HashMap<String, Entry>> {
        let mut url = self
            .endpoint
            .join("simple/token_price/solana")
            .context("price endpoint")?;
        let addresses = tokens
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        url.query_pairs_mut()
            .append_pair("contract_addresses", &addresses)
            .append_pair("vs_currencies", "sol");
        let mut request = self.client.get(url);
        if let Some(key) = &self.api_key {
            request = request.header(API_KEY_HEADER, key);
        }
        let response = request.send().await.context("price request")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("price request answered {status}: {body}"));
        }
        response.json().await.context("price response")
    }
}

/// A whole-token price in SOL converted to the scaled atom price:
/// `price * 10^(18 - decimals)`, saturating at `u64::MAX`. `None` when the
/// price rounds below one, those tokens count as unpriced.
fn scale(price: f64, decimals: u8) -> Option<u64> {
    let scaled = price * 10f64.powi(18 - i32::from(decimals));
    if scaled.is_nan() || scaled < 1.0 {
        return None;
    }
    Some(if scaled >= u64::MAX as f64 {
        u64::MAX
    } else {
        scaled as u64
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest},
        std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    fn config(endpoint: Url) -> config::NativePrices {
        config::NativePrices {
            endpoint,
            api_key: None,
            ttl: Duration::from_secs(60),
        }
    }

    /// Serve a fixed CoinGecko response, counting the requests.
    async fn coingecko(response: serde_json::Value) -> (Url, Arc<AtomicUsize>) {
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let app = axum::Router::new().route(
            "/simple/token_price/solana",
            axum::routing::get(move || {
                counter.fetch_add(1, Ordering::Relaxed);
                let response = response.clone();
                async move { axum::Json(response) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/").parse().unwrap(), requests)
    }

    #[test]
    fn scales_whole_token_prices_to_atoms() {
        // A 6-decimals token at 0.005 SOL: 0.005 * 10^12.
        assert_eq!(scale(0.005, 6), Some(5_000_000_000));
        // The native token itself: 1.0 * 10^9.
        assert_eq!(scale(1.0, 9), Some(Solana::NATIVE_PRICE_DENOMINATOR));
        assert_eq!(scale(0.0, 6), None);
        assert_eq!(scale(f64::NAN, 6), None);
        assert_eq!(scale(f64::MAX, 0), Some(u64::MAX));
    }

    /// The wrapped native mint is priced at the denominator without any
    /// lookup: the endpoint and the RPC here are dead.
    #[tokio::test]
    async fn prices_the_native_mint_locally() {
        let wrapped = Pubkey::new_unique();
        let prices = NativePrices::new(
            &config("http://127.0.0.1:1/".parse().unwrap()),
            SolanaRPC::new_mock_with_mocks(Mocks::default()),
            wrapped,
        );
        let result = prices.prices(HashSet::from([wrapped])).await.unwrap();
        assert_eq!(result[&wrapped], Solana::NATIVE_PRICE_DENOMINATOR);
    }

    /// A listed token is priced through its decimals, an unlisted one is
    /// absent, and the second call is served from the cache.
    #[tokio::test]
    async fn prices_and_caches_auction_tokens() {
        let listed = Pubkey::new_unique();
        let unlisted = Pubkey::new_unique();
        let (endpoint, requests) = coingecko(serde_json::json!({
            listed.to_string(): { "sol": 0.005 },
        }))
        .await;
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [
                    crate::tests::mint_account_json(6),
                    crate::tests::mint_account_json(6),
                ],
            }),
        )]);
        let prices = NativePrices::new(
            &config(endpoint),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let result = prices
            .prices(HashSet::from([listed, unlisted]))
            .await
            .unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
        assert_eq!(result.get(&unlisted), None);

        // Both mints are cached, the unlisted one negatively: the RPC mock
        // is consumed and the endpoint sees no second request.
        let result = prices
            .prices(HashSet::from([listed, unlisted]))
            .await
            .unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
        assert_eq!(requests.load(Ordering::Relaxed), 1);
    }

    /// A failing price endpoint fails the whole lookup.
    #[tokio::test]
    async fn fails_closed_on_endpoint_errors() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [crate::tests::mint_account_json(6)],
            }),
        )]);
        let prices = NativePrices::new(
            &config("http://127.0.0.1:1/".parse().unwrap()),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );
        assert!(
            prices
                .prices(HashSet::from([Pubkey::new_unique()]))
                .await
                .is_err()
        );
    }
}
