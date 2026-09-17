//! Native token prices for auction tokens, denominated in wSOL.

use {
    crate::infra::config,
    anyhow::{Context, Result, anyhow},
    chain_types::{ChainTypes, solana::Solana},
    cow_solana_rpc::SolanaRPC,
    futures::future::join_all,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
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

/// How long one driver quote may take before the token counts as unpriced by
/// that driver.
const DRIVER_QUOTE_TIMEOUT: Duration = Duration::from_secs(3);

/// Native price lookups for auction tokens, cached per token. The configured
/// estimators are asked in order: a token the first one does not price is
/// asked from the next.
pub struct NativePrices {
    sources: Vec<Source>,
    rpc: SolanaRPC,
    wrapped_native: Pubkey,
    ttl: Duration,
    /// Fetched prices by mint. `None` records a mint no estimator prices, so
    /// unlisted mints are not refetched every cut.
    prices: Mutex<HashMap<Pubkey, (Instant, Option<u64>)>>,
    /// Mint decimals never change, so they are cached forever.
    decimals: Mutex<HashMap<Pubkey, u8>>,
}

/// One configured price source.
enum Source {
    CoinGecko {
        client: reqwest::Client,
        endpoint: Url,
        api_key: Option<String>,
    },
    /// A solver driver quoted through its regular `/quote` route, like the
    /// EVM driver-backed native estimators.
    Driver {
        client: reqwest::Client,
        name: String,
        endpoint: Url,
    },
}

/// One priced entry of the CoinGecko `simple/token_price` response.
#[derive(Debug, Deserialize)]
struct Entry {
    sol: Option<f64>,
}

/// The driver `/quote` request: sell one whole token for wSOL.
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
    pub fn new(config: &config::NativePrices, rpc: SolanaRPC, wrapped_native: Pubkey) -> Self {
        let client = reqwest::Client::new();
        let sources = config
            .estimators
            .iter()
            .map(|estimator| match estimator {
                config::NativePriceEstimator::CoinGecko { endpoint, api_key } => {
                    Source::CoinGecko {
                        client: client.clone(),
                        endpoint: endpoint.clone(),
                        api_key: api_key.clone(),
                    }
                }
                config::NativePriceEstimator::Driver { name, url } => Source::Driver {
                    client: client.clone(),
                    name: name.clone(),
                    endpoint: url.clone(),
                },
            })
            .collect();
        Self {
            sources,
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
        }
    }

    /// The lamport value of one atom of each token, scaled by 10^9 like
    /// [`ChainTypes::NATIVE_PRICE_DENOMINATOR`]. Tokens no estimator prices
    /// are absent from the result. The call only fails when every estimator
    /// failed and nothing was priced: a partially priced auction is normal
    /// (unlisted tokens), a wholly unpriced one on a broken source is not
    /// worth ranking.
    pub async fn prices(&self, tokens: HashSet<Pubkey>) -> Result<HashMap<Pubkey, u64>> {
        let mut result = HashMap::new();
        let mut remaining = Vec::new();
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
                    _ => remaining.push(token),
                }
            }
        }
        if remaining.is_empty() {
            return Ok(result);
        }

        let decimals = self.decimals(&remaining).await?;
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
                            result.insert(*token, *price);
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
        // Tokens no source listed are cached negatively so they are not
        // refetched every cut, but only when every source answered: a failed
        // source might know them, so its cycle retries instead.
        if failures == 0 {
            let mut cache = self.prices.lock().expect("price cache poisoned");
            for token in remaining {
                cache.insert(token, (now, None));
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
                client, endpoint, ..
            } => driver(client, endpoint, tokens, decimals, wrapped_native).await,
        }
    }
}

/// One `simple/token_price` request for the given mints.
async fn coingecko(
    client: &reqwest::Client,
    endpoint: &Url,
    api_key: Option<&str>,
    tokens: &[Pubkey],
    decimals: &HashMap<Pubkey, u8>,
) -> Result<HashMap<Pubkey, u64>> {
    let mut url = endpoint
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
    let quoted: HashMap<String, Entry> = response.json().await.context("price response")?;
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

/// Quote one whole unit of each token into wSOL on the driver, concurrently.
/// A driver rejection prices nothing for that token (no route is a routine
/// answer), a transport failure fails the source.
async fn driver(
    client: &reqwest::Client,
    endpoint: &Url,
    tokens: &[Pubkey],
    decimals: &HashMap<Pubkey, u8>,
    wrapped_native: Pubkey,
) -> Result<HashMap<Pubkey, u64>> {
    let url = endpoint.join("quote").context("quote endpoint")?;
    let quotes = join_all(tokens.iter().map(|token| {
        let url = url.clone();
        async move {
            let probe = 10u64.checked_pow(u32::from(decimals[token]))?;
            let request = QuoteRequest {
                sell_token: *token,
                buy_token: wrapped_native,
                amount: probe,
                kind: "sell",
                deadline: chrono::Utc::now() + DRIVER_QUOTE_TIMEOUT,
            };
            let response = client
                .post(url)
                .json(&request)
                .timeout(DRIVER_QUOTE_TIMEOUT)
                .send()
                .await
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            let quote: QuoteResponse = response.json().await.ok()?;
            Some((*token, quote))
        }
    }))
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
            let price = u64::try_from(price).unwrap_or(u64::MAX);
            (price > 0).then_some((token, price))
        })
        .collect())
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

    fn coingecko_config(endpoint: Url) -> config::NativePrices {
        config::NativePrices {
            estimators: vec![config::NativePriceEstimator::CoinGecko {
                endpoint,
                api_key: None,
            }],
            ttl: Duration::from_secs(60),
        }
    }

    fn mint_mocks(mints: usize) -> Mocks {
        Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!({
                "context": {"slot": 1u64, "apiVersion": "2.0.0"},
                "value": (0..mints).map(|_| crate::tests::mint_account_json(6)).collect::<Vec<_>>(),
            }),
        )])
    }

    /// Serve a fixed CoinGecko response, counting the requests.
    async fn coingecko_server(response: serde_json::Value) -> (Url, Arc<AtomicUsize>) {
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

    /// Serve fixed driver quotes: `buy_amount` lamports for any request of
    /// the recorded `sell_amount`.
    async fn driver_server(buy_amount: u64) -> Url {
        let app = axum::Router::new().route(
            "/quote",
            axum::routing::post(
                move |axum::Json(request): axum::Json<serde_json::Value>| async move {
                    axum::Json(serde_json::json!({
                        "sellAmount": request["amount"],
                        "buyAmount": buy_amount.to_string(),
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
        assert_eq!(scale(f64::NAN, 6), None);
        assert_eq!(scale(f64::MAX, 0), Some(u64::MAX));
    }

    /// The wrapped native mint is priced at the denominator without any
    /// lookup: the estimators here are gone and the RPC is dead.
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

    /// A token CoinGecko does not list falls through to the driver, which
    /// prices it by quoting one whole token into wSOL.
    #[tokio::test]
    async fn falls_back_to_the_driver_source() {
        let token = Pubkey::new_unique();
        let (coingecko, _) = coingecko_server(serde_json::json!({})).await;
        // One whole 6-decimals token buys 0.005 SOL.
        let driver = driver_server(5_000_000).await;
        let config = config::NativePrices {
            estimators: vec![
                config::NativePriceEstimator::CoinGecko {
                    endpoint: coingecko,
                    api_key: None,
                },
                config::NativePriceEstimator::Driver {
                    name: "baseline".to_owned(),
                    url: driver,
                },
            ],
            ttl: Duration::from_secs(60),
        };
        let prices = NativePrices::new(
            &config,
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        // 5_000_000 lamports per 10^6 atoms, scaled by 10^9.
        assert_eq!(result.get(&token), Some(&5_000_000_000));
    }

    /// With every estimator down the lookup fails, with one of them down it
    /// degrades to the answering ones.
    #[tokio::test]
    async fn fails_closed_only_when_every_source_fails() {
        let token = Pubkey::new_unique();
        let dead = config::NativePriceEstimator::CoinGecko {
            endpoint: "http://127.0.0.1:1/".parse().unwrap(),
            api_key: None,
        };
        let prices = NativePrices::new(
            &config::NativePrices {
                estimators: vec![dead.clone()],
                ttl: Duration::from_secs(60),
            },
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        assert!(prices.prices(HashSet::from([token])).await.is_err());

        let driver = driver_server(5_000_000).await;
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
            },
            SolanaRPC::new_mock_with_mocks(mint_mocks(1)),
            Pubkey::new_unique(),
        );
        let result = prices.prices(HashSet::from([token])).await.unwrap();
        assert_eq!(result.get(&token), Some(&5_000_000_000));
    }
}
