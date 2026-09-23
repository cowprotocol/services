//! Native token prices for auction tokens, denominated in wSOL.

use {
    crate::infra::config,
    anyhow::{Context, Result, anyhow},
    chain_types::{ChainTypes, solana::Solana},
    cow_solana_rpc::SolanaRPC,
    futures::{StreamExt, stream},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
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

/// Ceiling on one request to a price source, so a hung endpoint cannot stall
/// the auction cut.
const SOURCE_TIMEOUT: Duration = Duration::from_secs(3);

/// Ceiling on one driver probe quote, which costs the solver a route search
/// and so runs longer than a price API answer. Kept under the solve deadline
/// the same solvers work to.
const DRIVER_TIMEOUT: Duration = Duration::from_secs(5);

/// Driver quotes in flight at once, bounding the load one lookup puts on a
/// single driver.
const DRIVER_CONCURRENCY: usize = 10;

/// Mints per `getMultipleAccounts` request, the RPC method's cap.
const ACCOUNTS_CHUNK: usize = 100;

/// Mints per CoinGecko request, the most `simple/token_price` prices in one
/// answer. The denominator needs no slot: wSOL prices at the denominator
/// without being asked.
const PRICES_CHUNK: usize = 20;

/// Native price lookups for auction tokens, cached per token. The configured
/// estimators are asked in order: a token the first one does not price is
/// asked from the next. A background task keeps the tokens of the latest
/// lookup fresh, so a cut normally reads the cache instead of waiting on a
/// source. Without sources every token prices at the native denominator.
pub enum NativePrices {
    /// No sources configured: every token prices at the native denominator
    /// and nothing is fetched.
    Denominated,
    Configured(Arc<Inner>),
}

pub struct Inner {
    sources: Vec<Source>,
    rpc: SolanaRPC,
    wrapped_native: Pubkey,
    ttl: Duration,
    /// Fetched prices by mint. `None` records a mint no estimator prices, so
    /// unpriced mints are not refetched every cut.
    prices: Mutex<HashMap<Pubkey, (Instant, Option<u64>)>>,
    /// Mint decimals never change, so they are cached forever.
    decimals: Mutex<HashMap<Pubkey, u8>>,
    /// The tokens of the latest lookup, the set the refresher keeps fresh.
    maintained: Mutex<HashSet<Pubkey>>,
}

/// One configured price source.
enum Source {
    CoinGecko {
        client: reqwest::Client,
        endpoint: Url,
        api_key: Option<String>,
    },
    /// A solver driver quoted through its regular `/quote` route.
    Driver {
        client: reqwest::Client,
        name: String,
        endpoint: Url,
        /// Lamports bought per probe quote.
        probe_amount: u64,
    },
}

/// One priced entry of the CoinGecko `simple/token_price` response.
#[derive(Debug, Deserialize)]
struct Entry {
    sol: Option<f64>,
}

/// The driver `/quote` request: buy the probe amount of wSOL with the token.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteRequest {
    #[serde_as(as = "DisplayFromStr")]
    sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    kind: &'static str,
    deadline: chrono::DateTime<chrono::Utc>,
}

/// The driver `/quote` response fields the price needs.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteResponse {
    #[serde_as(as = "DisplayFromStr")]
    sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    buy_amount: u64,
}

impl NativePrices {
    /// Builds the lookup and spawns its refresher. Must run inside a tokio
    /// runtime. Without configured sources the lookup prices every token at
    /// the denominator instead.
    pub fn new(config: &config::NativePrices, rpc: SolanaRPC, wrapped_native: Pubkey) -> Self {
        if config.estimators.is_empty() {
            tracing::warn!(
                "no native price sources configured, pricing every token at the denominator"
            );
            return Self::Denominated;
        }
        let client = reqwest::Client::builder()
            .timeout(SOURCE_TIMEOUT)
            .build()
            .expect("reqwest client");
        let sources = config
            .estimators
            .iter()
            .map(|estimator| match estimator {
                config::NativePriceEstimator::CoinGecko { endpoint, api_key } => {
                    Source::CoinGecko {
                        client: client.clone(),
                        endpoint: endpoint.clone(),
                        api_key: Some(api_key.clone()).filter(|key| !key.is_empty()),
                    }
                }
                config::NativePriceEstimator::Driver { name, url } => Source::Driver {
                    client: client.clone(),
                    name: name.clone(),
                    endpoint: url.clone(),
                    probe_amount: config.driver_probe_lamports,
                },
            })
            .collect();
        let inner = Arc::new(Inner {
            sources,
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
        Self::Configured(inner)
    }

    /// The lamport value of one atom of each token, scaled by 10^9 like
    /// [`ChainTypes::NATIVE_PRICE_DENOMINATOR`]. A token no estimator prices,
    /// or whose mint does not read, is absent from the result: solutions
    /// trading it score nothing. The call only fails when every estimator
    /// failed: a partially priced auction is normal, a wholly unpriced one on
    /// broken sources is not worth ranking.
    pub async fn prices(&self, tokens: HashSet<Pubkey>) -> Result<HashMap<Pubkey, u64>> {
        match self {
            Self::Denominated => Ok(tokens
                .into_iter()
                .map(|token| (token, Solana::NATIVE_PRICE_DENOMINATOR))
                .collect()),
            Self::Configured(inner) => inner.prices(tokens).await,
        }
    }

    /// A lookup pre-seeded for tests: the given prices never expire, nothing
    /// is fetched, and no refresher runs.
    #[cfg(test)]
    pub(crate) fn seeded(entries: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        Self::Configured(Arc::new(Inner {
            sources: Vec::new(),
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

    /// Ask the sources in order for the tokens none of the earlier ones
    /// priced, and cache every verdict.
    async fn fetch_into_cache(&self, tokens: &[Pubkey]) -> Result<()> {
        let decimals = self.decimals(tokens).await?;
        // A token whose mint did not resolve cannot be scaled, so it counts
        // as unpriced until its entry expires.
        let (mut remaining, unpriceable): (Vec<_>, Vec<_>) = tokens
            .iter()
            .copied()
            .partition(|token| decimals.contains_key(token));
        let now = Instant::now();
        {
            let mut cache = self.prices.lock().expect("price cache poisoned");
            for token in unpriceable {
                cache.insert(token, (now, None));
            }
        }
        if remaining.is_empty() {
            return Ok(());
        }

        let mut failures = 0;
        for source in &self.sources {
            if remaining.is_empty() {
                break;
            }
            match source
                .fetch(&remaining, &decimals, self.wrapped_native)
                .await
            {
                Ok(priced) => {
                    let mut cache = self.prices.lock().expect("price cache poisoned");
                    remaining.retain(|token| match priced.get(token) {
                        Some(price) => {
                            cache.insert(*token, (now, Some(*price)));
                            false
                        }
                        None => true,
                    });
                }
                Err(err) => {
                    failures += 1;
                    tracing::warn!(source = source.name(), ?err, "price source failed");
                }
            }
        }
        if failures == self.sources.len() && !self.sources.is_empty() {
            return Err(anyhow!("every native price source failed"));
        }
        // Tokens no source priced are cached negatively so they are not
        // refetched every cut, but only when every source answered: a failed
        // source might know them, so its cycle retries instead.
        if failures == 0 {
            let mut cache = self.prices.lock().expect("price cache poisoned");
            for token in remaining {
                cache.insert(token, (now, None));
            }
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
}

impl Source {
    fn name(&self) -> &str {
        match self {
            Self::CoinGecko { .. } => "CoinGecko",
            Self::Driver { name, .. } => name,
        }
    }

    /// The scaled prices of the tokens this source can price. Tokens it
    /// cannot price are absent, a transport or protocol failure fails the
    /// whole source.
    async fn fetch(
        &self,
        tokens: &[Pubkey],
        decimals: &HashMap<Pubkey, u8>,
        wrapped_native: Pubkey,
    ) -> Result<HashMap<Pubkey, u64>> {
        match self {
            Self::CoinGecko {
                client,
                endpoint,
                api_key,
            } => coingecko(client, endpoint, api_key.as_deref(), tokens, decimals).await,
            Self::Driver {
                client,
                endpoint,
                probe_amount,
                ..
            } => driver(client, endpoint, tokens, wrapped_native, *probe_amount).await,
        }
    }
}

/// Append a path to a configured base URL. `Url::join` resolves relative to
/// the last slash and would drop a final path segment of a base configured
/// without a trailing slash.
fn route(base: &Url, path: &str) -> Result<Url> {
    Url::parse(&format!("{}/{path}", base.as_str().trim_end_matches('/'))).context("source url")
}

/// The `simple/token_price` prices for the given mints, requested in chunks.
/// The configured endpoint addresses the route, so only the chain is appended.
async fn coingecko(
    client: &reqwest::Client,
    endpoint: &Url,
    api_key: Option<&str>,
    tokens: &[Pubkey],
    decimals: &HashMap<Pubkey, u8>,
) -> Result<HashMap<Pubkey, u64>> {
    let base = route(endpoint, "solana")?;
    // A caching proxy in front of the API keys on the URL, so the mint order
    // must not vary between lookups of the same token set.
    let mut sorted = tokens.to_vec();
    sorted.sort();
    let mut quoted: HashMap<String, Entry> = HashMap::new();
    for chunk in sorted.chunks(PRICES_CHUNK) {
        let mut url = base.clone();
        let addresses = chunk
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        url.query_pairs_mut()
            .append_pair("contract_addresses", &addresses)
            .append_pair("vs_currencies", "sol")
            .append_pair("precision", "full");
        let mut request = client.get(url);
        if let Some(key) = api_key {
            request = request.header(API_KEY_HEADER, key);
        }
        let response = request.send().await.context("price request")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("price request answered {status}: {body}"));
        }
        quoted.extend(
            response
                .json::<HashMap<String, Entry>>()
                .await
                .context("price response")?,
        );
    }
    // An answer naming none of the asked mints is contract drift (a changed
    // key format, for example), not a market answer. Failing the source keeps
    // the drift loud and lets the next source try.
    if !quoted.is_empty()
        && tokens
            .iter()
            .all(|token| !quoted.contains_key(&token.to_string()))
    {
        return Err(anyhow!("price response keys match no requested mint"));
    }
    Ok(tokens
        .iter()
        .filter_map(|token| {
            let price = quoted
                .get(&token.to_string())
                .and_then(|entry| entry.sol)
                .and_then(|sol| scale(sol, decimals[token]))?;
            Some((*token, price))
        })
        .collect())
}

/// Buy a fixed amount of wSOL with each token on the driver. The probe is
/// denominated in the native token so its economic size is the same for every
/// token, whatever one whole unit of it is worth. A driver rejection prices
/// nothing for that token (no route is a routine answer), a transport failure
/// fails the source.
async fn driver(
    client: &reqwest::Client,
    endpoint: &Url,
    tokens: &[Pubkey],
    wrapped_native: Pubkey,
    probe_amount: u64,
) -> Result<HashMap<Pubkey, u64>> {
    let url = route(endpoint, "quote")?;
    let quotes: Vec<_> = stream::iter(tokens.iter().copied().map(|token| {
        let url = url.clone();
        async move {
            let request = QuoteRequest {
                sell_token: token,
                buy_token: wrapped_native,
                amount: probe_amount,
                kind: "buy",
                deadline: chrono::Utc::now() + DRIVER_TIMEOUT,
            };
            let response = client
                .post(url)
                .json(&request)
                .timeout(DRIVER_TIMEOUT)
                .send()
                .await
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            let quote: QuoteResponse = response.json().await.ok()?;
            Some((token, quote))
        }
    }))
    .buffer_unordered(DRIVER_CONCURRENCY)
    .collect()
    .await;
    // Every quote failing while several tokens were asked reads as the
    // driver being down rather than uniformly missing routes.
    if quotes.iter().all(Option::is_none) {
        return Err(anyhow!("no quote succeeded"));
    }
    Ok(quotes
        .into_iter()
        .flatten()
        .filter_map(|(token, quote)| {
            // `buy_amount` is in lamports, `sell_amount` in token atoms, and
            // the stored price is lamports per atom scaled by 10^9.
            let price = (u128::from(quote.buy_amount) * 1_000_000_000)
                .checked_div(u128::from(quote.sell_amount))?;
            // A price outside the scaled range counts as unpriced: clamping
            // would let it outrank every real one.
            let price = u64::try_from(price).ok()?;
            (price > 0).then_some((token, price))
        })
        .collect())
}

/// A whole-token price in SOL converted to the scaled atom price:
/// `price * 10^(18 - decimals)`. `None` for a price that is not a positive
/// real number, rounds below one atom of value, or does not fit the scaled
/// range: those tokens count as unpriced. Clamping instead would let a
/// nonsense price outrank every real one.
fn scale(price: f64, decimals: u8) -> Option<u64> {
    if !price.is_normal() || price <= 0.0 {
        return None;
    }
    let scaled = price * 10f64.powi(18 - i32::from(decimals));
    if !scaled.is_finite() || scaled < 1.0 || scaled >= u64::MAX as f64 {
        return None;
    }
    Some(scaled as u64)
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

    fn coingecko_config(endpoint: Url) -> config::NativePrices {
        config::NativePrices {
            estimators: vec![config::NativePriceEstimator::CoinGecko {
                endpoint,
                api_key: String::new(),
            }],
            ttl: Duration::from_secs(60),
            driver_probe_lamports: PROBE_LAMPORTS,
        }
    }

    /// The probe the driver tests quote with: a tenth of a SOL.
    const PROBE_LAMPORTS: u64 = 100_000_000;

    fn mint_mocks(mints: usize) -> Mocks {
        Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": (0..mints).map(|_| crate::tests::mint_account_json(6)).collect::<Vec<_>>(),
            }),
        )])
    }

    /// Serve a fixed CoinGecko response at the given path, counting requests.
    async fn coingecko_server_at(
        path: &str,
        response: serde_json::Value,
    ) -> (Url, Arc<AtomicUsize>) {
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let app = axum::Router::new().route(
            path,
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

    async fn coingecko_server(response: serde_json::Value) -> (Url, Arc<AtomicUsize>) {
        let (root, requests) = coingecko_server_at("/simple/token_price/solana", response).await;
        (
            format!("{root}simple/token_price").parse().unwrap(),
            requests,
        )
    }

    /// Serve one fixed driver quote: `sell_amount` token atoms buy the
    /// probe the request asks for.
    async fn driver_server(sell_amount: u64) -> Url {
        let app = axum::Router::new().route(
            "/quote",
            axum::routing::post(
                move |axum::Json(request): axum::Json<serde_json::Value>| async move {
                    assert_eq!(request["kind"], "buy", "the probe buys the native token");
                    axum::Json(serde_json::json!({
                        "sellAmount": sell_amount.to_string(),
                        "buyAmount": request["amount"],
                        "solver": Pubkey::new_unique().to_string(),
                    }))
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/").parse().unwrap()
    }

    #[test]
    fn scales_whole_token_prices_to_atoms() {
        // A 6-decimals token at 0.005 SOL: 0.005 * 10^12.
        assert_eq!(scale(0.005, 6), Some(5_000_000_000));
        // The native token itself: 1.0 * 10^9.
        assert_eq!(scale(1.0, 9), Some(Solana::NATIVE_PRICE_DENOMINATOR));
        assert_eq!(scale(0.0, 6), None);
        assert_eq!(scale(-1.0, 6), None);
        assert_eq!(scale(f64::NAN, 6), None);
        assert_eq!(scale(f64::INFINITY, 6), None);
        // Beyond the scaled range: unpriced beats ranking first on nonsense.
        assert_eq!(scale(f64::MAX, 0), None);
        assert_eq!(scale(1e9, 0), None);
    }

    /// The wrapped native mint is priced at the denominator without any
    /// lookup: the estimators here are dead and so is the RPC.
    #[tokio::test]
    async fn prices_the_native_mint_locally() {
        let wrapped = Pubkey::new_unique();
        let prices = NativePrices::new(
            &coingecko_config("http://127.0.0.1:1/".parse().unwrap()),
            SolanaRPC::new_mock_with_mocks(Mocks::default()),
            wrapped,
        );
        let result = prices.prices(HashSet::from([wrapped])).await.unwrap();
        assert_eq!(result[&wrapped], Solana::NATIVE_PRICE_DENOMINATOR);
    }

    /// Without sources every token prices at the denominator and nothing is
    /// fetched: the RPC here is dead.
    #[tokio::test]
    async fn prices_at_the_denominator_without_sources() {
        let token = Pubkey::new_unique();
        let prices = NativePrices::new(
            &config::NativePrices {
                estimators: Vec::new(),
                ttl: Duration::from_secs(60),
                driver_probe_lamports: PROBE_LAMPORTS,
            },
            SolanaRPC::new_mock_with_mocks(Mocks::default()),
            Pubkey::new_unique(),
        );
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert_eq!(result[&token], Solana::NATIVE_PRICE_DENOMINATOR);
    }

    /// A listed token is priced through its decimals, an unlisted one is
    /// absent, and the second call is served from the cache.
    #[tokio::test]
    async fn prices_and_caches_auction_tokens() {
        let listed = Pubkey::new_unique();
        let unlisted = Pubkey::new_unique();
        let (endpoint, requests) = coingecko_server(serde_json::json!({
            listed.to_string(): { "sol": 0.005 },
        }))
        .await;
        let prices = NativePrices::new(
            &coingecko_config(endpoint),
            SolanaRPC::new_mock_with_mocks(mint_mocks(2)),
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
        let (endpoint, _) = coingecko_server_at(
            "/api/v3/simple/token_price/solana",
            serde_json::json!({ listed.to_string(): { "sol": 0.005 } }),
        )
        .await;
        let endpoint = format!("{}api/v3/simple/token_price", endpoint.as_str())
            .parse()
            .unwrap();
        let prices = NativePrices::new(
            &coingecko_config(endpoint),
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );

        let result = prices.prices(HashSet::from([listed])).await.unwrap();
        assert_eq!(result.get(&listed), Some(&5_000_000_000));
    }

    /// A mint that does not unpack counts as unpriced: the lookup succeeds
    /// without it and the verdict is cached.
    #[tokio::test]
    async fn unreadable_mints_count_as_unpriced() {
        let token = Pubkey::new_unique();
        let (endpoint, requests) = coingecko_server(serde_json::json!({})).await;
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
            &coingecko_config(endpoint),
            SolanaRPC::new_mock_with_mocks(mocks),
            Pubkey::new_unique(),
        );

        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert!(result.is_empty());
        // Nothing was priceable, so no source was asked, and the verdict is
        // cached: the consumed RPC mock would fail a refetch.
        assert_eq!(requests.load(Ordering::Relaxed), 0);
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert!(result.is_empty());
    }

    /// The refresher refetches the maintained token on its own, and the next
    /// lookup is served from the refreshed cache.
    #[tokio::test]
    async fn refreshes_maintained_prices_in_the_background() {
        let listed = Pubkey::new_unique();
        let (endpoint, requests) = coingecko_server(serde_json::json!({
            listed.to_string(): { "sol": 0.005 },
        }))
        .await;
        let prices = NativePrices::new(
            &config::NativePrices {
                estimators: vec![config::NativePriceEstimator::CoinGecko {
                    endpoint,
                    api_key: String::new(),
                }],
                ttl: Duration::from_millis(600),
                driver_probe_lamports: PROBE_LAMPORTS,
            },
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
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

    /// A token CoinGecko does not list falls through to the driver, which
    /// prices it by quoting one whole token into wSOL.
    #[tokio::test]
    async fn falls_back_to_the_driver_source() {
        let token = Pubkey::new_unique();
        let (coingecko, _) = coingecko_server(serde_json::json!({})).await;
        // The 0.1 SOL probe costs 20 whole 6-decimals tokens, so one token is
        // worth 0.005 SOL.
        let driver = driver_server(20_000_000).await;
        let config = config::NativePrices {
            estimators: vec![
                config::NativePriceEstimator::CoinGecko {
                    endpoint: coingecko,
                    api_key: String::new(),
                },
                config::NativePriceEstimator::Driver {
                    name: "baseline".to_owned(),
                    url: driver,
                },
            ],
            ttl: Duration::from_secs(60),
            driver_probe_lamports: PROBE_LAMPORTS,
        };
        let prices = NativePrices::new(
            &config,
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        // 10^8 lamports per 2*10^7 atoms, scaled by 10^9.
        assert_eq!(result.get(&token), Some(&5_000_000_000));
    }

    /// With every estimator down the lookup fails, with one of them down it
    /// degrades to the answering ones.
    #[tokio::test]
    async fn fails_closed_only_when_every_source_fails() {
        let token = Pubkey::new_unique();
        let dead = config::NativePriceEstimator::CoinGecko {
            endpoint: "http://127.0.0.1:1/".parse().unwrap(),
            api_key: String::new(),
        };
        let prices = NativePrices::new(
            &config::NativePrices {
                estimators: vec![dead.clone()],
                ttl: Duration::from_secs(60),
                driver_probe_lamports: PROBE_LAMPORTS,
            },
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        assert!(prices.prices(HashSet::from([token])).await.is_err());

        let driver = driver_server(20_000_000).await;
        let prices = NativePrices::new(
            &config::NativePrices {
                estimators: vec![
                    dead,
                    config::NativePriceEstimator::Driver {
                        name: "baseline".to_owned(),
                        url: driver,
                    },
                ],
                ttl: Duration::from_secs(60),
                driver_probe_lamports: PROBE_LAMPORTS,
            },
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert_eq!(result.get(&token), Some(&5_000_000_000));
    }
}
