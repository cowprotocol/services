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
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    url::Url,
};

/// CoinGecko's authorization header for Pro plans.
const API_KEY_HEADER: &str = "x-cg-pro-api-key";

/// Mints per `getMultipleAccounts` request, the RPC method's cap.
const ACCOUNTS_CHUNK: usize = 100;

/// Mints per price request, keeping the `contract_addresses` parameter and
/// the URL bounded.
const PRICES_CHUNK: usize = 100;

/// Native price lookups for auction tokens, cached per token. A background
/// task keeps the tokens of the latest lookup fresh, so a cut normally reads
/// the cache instead of waiting on the price endpoint.
pub struct NativePrices(Arc<Inner>);

struct Inner {
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
    /// The tokens of the latest lookup, the set the refresher keeps fresh.
    maintained: Mutex<HashSet<Pubkey>>,
}

/// One priced entry of the CoinGecko `simple/token_price` response.
#[derive(Debug, Deserialize)]
struct Entry {
    sol: Option<f64>,
}

impl NativePrices {
    /// Builds the lookup and spawns its refresher. Must run inside a tokio
    /// runtime.
    pub fn new(config: &config::NativePrices, rpc: SolanaRPC, wrapped_native: Pubkey) -> Self {
        let inner = Arc::new(Inner {
            client: reqwest::Client::new(),
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            rpc,
            wrapped_native,
            ttl: config.ttl,
            prices: Mutex::new(HashMap::new()),
            decimals: Mutex::new(HashMap::new()),
            maintained: Mutex::new(HashSet::new()),
        });
        let refresher = Arc::clone(&inner);
        tokio::spawn(async move {
            // Ticking well inside the refresh margin keeps an entry from
            // expiring between two passes.
            let tick = (refresher.ttl / 6).max(Duration::from_millis(50));
            loop {
                tokio::time::sleep(tick).await;
                refresher.refresh().await;
            }
        });
        Self(inner)
    }

    /// The lamport value of one atom of each token, scaled by 10^9 like
    /// [`ChainTypes::NATIVE_PRICE_DENOMINATOR`]. A token without a listing
    /// or without a readable mint is absent from the result: solutions
    /// trading it score nothing. A failed lookup fails the whole call:
    /// whether the remaining prices still hold is unknowable, and a wrongly
    /// ranked auction is worse than none.
    pub async fn prices(&self, tokens: HashSet<Pubkey>) -> Result<HashMap<Pubkey, u64>> {
        self.0.prices(tokens).await
    }

    /// A lookup pre-seeded for tests: the given prices never expire, nothing
    /// is fetched, and no refresher runs.
    #[cfg(test)]
    pub(crate) fn seeded(entries: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        Self(Arc::new(Inner {
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
            maintained: Mutex::new(HashSet::new()),
        }))
    }
}

impl Inner {
    /// One refresher pass: refetch the maintained tokens nearing expiry, so
    /// lookups keep hitting fresh entries. A failed pass only logs, the
    /// entries then expire and the next lookup fetches inline.
    async fn refresh(&self) {
        let margin = self.ttl / 3;
        let now = Instant::now();
        let expiring: Vec<Pubkey> = {
            let maintained = self.maintained.lock().expect("maintained set poisoned");
            let cache = self.prices.lock().expect("price cache poisoned");
            maintained
                .iter()
                .filter(|token| {
                    cache.get(token).is_none_or(|(fetched, _)| {
                        now.duration_since(*fetched) + margin >= self.ttl
                    })
                })
                .copied()
                .collect()
        };
        if expiring.is_empty() {
            return;
        }
        if let Err(err) = self.fetch_into_cache(&expiring).await {
            tracing::warn!(?err, "native price refresh failed");
        }
    }

    async fn prices(&self, tokens: HashSet<Pubkey>) -> Result<HashMap<Pubkey, u64>> {
        let mut result = HashMap::new();
        let mut fetch = Vec::new();
        let now = Instant::now();
        {
            let cache = self.prices.lock().expect("price cache poisoned");
            for token in &tokens {
                if *token == self.wrapped_native {
                    result.insert(*token, Solana::NATIVE_PRICE_DENOMINATOR);
                    continue;
                }
                match cache.get(token) {
                    Some((fetched, price)) if now.duration_since(*fetched) < self.ttl => {
                        if let Some(price) = price {
                            result.insert(*token, *price);
                        }
                    }
                    _ => fetch.push(*token),
                }
            }
        }
        {
            // The refresher maintains what the latest lookup asked for.
            let mut maintained = self.maintained.lock().expect("maintained set poisoned");
            *maintained = tokens
                .into_iter()
                .filter(|token| *token != self.wrapped_native)
                .collect();
        }
        if fetch.is_empty() {
            return Ok(result);
        }

        self.fetch_into_cache(&fetch).await?;
        let cache = self.prices.lock().expect("price cache poisoned");
        for token in fetch {
            if let Some((_, Some(price))) = cache.get(&token) {
                result.insert(token, *price);
            }
        }
        Ok(result)
    }

    /// Fetch the tokens' prices and cache every answer, the missing ones
    /// negatively.
    async fn fetch_into_cache(&self, tokens: &[Pubkey]) -> Result<()> {
        let decimals = self.decimals(tokens).await?;
        // A token whose mint did not resolve cannot be scaled, so it prices
        // as unlisted until its entry expires.
        let (priceable, unpriceable): (Vec<_>, Vec<_>) = tokens
            .iter()
            .copied()
            .partition(|token| decimals.contains_key(token));
        {
            let now = Instant::now();
            let mut cache = self.prices.lock().expect("price cache poisoned");
            for token in unpriceable {
                cache.insert(token, (now, None));
            }
        }
        if priceable.is_empty() {
            return Ok(());
        }
        let quoted = self.fetch(&priceable).await?;
        // A non-empty answer naming none of the asked mints is contract
        // drift (a changed key format, for example), not a market answer.
        // Failing keeps the drift loud instead of caching every token as
        // unlisted.
        if !quoted.is_empty()
            && priceable
                .iter()
                .all(|token| !quoted.contains_key(&token.to_string()))
        {
            return Err(anyhow!("price response keys match no requested mint"));
        }
        let now = Instant::now();
        let mut cache = self.prices.lock().expect("price cache poisoned");
        for token in priceable {
            let price = quoted
                .get(&token.to_string())
                .and_then(|entry| entry.sol)
                .and_then(|sol| scale(sol, decimals[&token]));
            cache.insert(token, (now, price));
        }
        Ok(())
    }

    /// Decimals per mint, from the cache or the mint accounts on chain. A
    /// mint that is missing or does not unpack (a token-2022 mint with
    /// extensions, for example) is absent from the result: one odd token
    /// must not fail the price lookup and with it every auction cut.
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
        let mut accounts = HashMap::new();
        for chunk in fetch.chunks(ACCOUNTS_CHUNK) {
            accounts.extend(
                self.rpc
                    .multiple_accounts(chunk.iter().copied())
                    .await
                    .context("fetch mint accounts")?,
            );
        }
        let mut cache = self.decimals.lock().expect("decimals cache poisoned");
        for token in fetch {
            let Some(account) = accounts.get(&token) else {
                tracing::warn!(%token, "mint account not found, token unpriced");
                continue;
            };
            match Mint::unpack(&account.data) {
                Ok(mint) => {
                    cache.insert(token, mint.decimals);
                    result.insert(token, mint.decimals);
                }
                Err(err) => {
                    tracing::warn!(%token, ?err, "mint does not unpack, token unpriced");
                }
            }
        }
        Ok(result)
    }

    /// The `simple/token_price` entries for the given mints, requested in
    /// chunks.
    async fn fetch(&self, tokens: &[Pubkey]) -> Result<HashMap<String, Entry>> {
        // `Url::join` resolves relative to the last slash and would drop a
        // final path segment of an endpoint configured without a trailing
        // slash, so the path is appended textually.
        let base = Url::parse(&format!(
            "{}/simple/token_price/solana",
            self.endpoint.as_str().trim_end_matches('/')
        ))
        .context("price endpoint")?;
        let mut result = HashMap::new();
        for chunk in tokens.chunks(PRICES_CHUNK) {
            let mut url = base.clone();
            let addresses = chunk
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
            let entries: HashMap<String, Entry> =
                response.json().await.context("price response")?;
            result.extend(entries);
        }
        Ok(result)
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

    /// An endpoint configured without a trailing slash keeps its base path.
    #[tokio::test]
    async fn endpoint_path_survives_a_missing_trailing_slash() {
        let listed = Pubkey::new_unique();
        let response = serde_json::json!({ listed.to_string(): { "sol": 0.005 } });
        let app = axum::Router::new().route(
            "/api/v3/simple/token_price/solana",
            axum::routing::get(move || {
                let response = response.clone();
                async move { axum::Json(response) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [crate::tests::mint_account_json(6)],
            }),
        )]);
        let prices = NativePrices::new(
            &config(format!("http://{addr}/api/v3").parse().unwrap()),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let result = prices.prices(HashSet::from([listed])).await.unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
    }

    /// An answer naming none of the asked mints fails the lookup instead of
    /// negatively caching every token.
    #[tokio::test]
    async fn fails_when_response_keys_match_no_mint() {
        let token = Pubkey::new_unique();
        let (endpoint, _) = coingecko(serde_json::json!({
            "someotherkey": { "sol": 0.005 },
        }))
        .await;
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [crate::tests::mint_account_json(6)],
            }),
        )]);
        let prices = NativePrices::new(
            &config(endpoint),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        assert!(prices.prices(HashSet::from([token])).await.is_err());
    }

    /// The refresher refetches the maintained token on its own, and the next
    /// lookup is served from the refreshed cache.
    #[tokio::test]
    async fn refreshes_maintained_prices_in_the_background() {
        let listed = Pubkey::new_unique();
        let (endpoint, requests) = coingecko(serde_json::json!({
            listed.to_string(): { "sol": 0.005 },
        }))
        .await;
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [crate::tests::mint_account_json(6)],
            }),
        )]);
        let prices = NativePrices::new(
            &config::NativePrices {
                endpoint,
                api_key: None,
                ttl: Duration::from_millis(600),
            },
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let result = prices.prices(HashSet::from([listed])).await.unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
        assert_eq!(requests.load(Ordering::Relaxed), 1);

        // The background pass refetches without another lookup.
        for _ in 0..200 {
            if requests.load(Ordering::Relaxed) >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(requests.load(Ordering::Relaxed) >= 2);
        let result = prices.prices(HashSet::from([listed])).await.unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
    }

    /// A mint that does not unpack prices as unlisted: the lookup succeeds
    /// without it and the verdict is cached.
    #[tokio::test]
    async fn unreadable_mints_price_as_unlisted() {
        let token = Pubkey::new_unique();
        let (endpoint, requests) = coingecko(serde_json::json!({})).await;
        // A one-byte account is no mint layout.
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": [{
                    "lamports": 1u64,
                    "data": ["AA==", "base64"],
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "executable": false,
                    "rentEpoch": 0u64,
                    "space": 1u64,
                }],
            }),
        )]);
        let prices = NativePrices::new(
            &config(endpoint),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert!(result.is_empty());
        // Nothing was priceable, so the price endpoint was never asked, and
        // the verdict is cached: the consumed RPC mock would fail a refetch.
        assert_eq!(requests.load(Ordering::Relaxed), 0);
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert!(result.is_empty());
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
