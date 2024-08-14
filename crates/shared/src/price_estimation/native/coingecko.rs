use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{buffered::NativePriceBatchFetching, PriceEstimationError},
    anyhow::{anyhow, Context, Result},
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{Client, StatusCode},
    rust_decimal::{prelude::ToPrimitive, Decimal},
    serde::Deserialize,
    std::collections::{HashMap, HashSet},
    url::Url,
};

#[derive(Debug, Deserialize)]
struct Response(HashMap<H160, Price>);

#[derive(Debug, Deserialize)]
struct Price {
    eth: f64,
}

type Token = H160;

pub struct CoinGecko {
    client: Client,
    base_url: Url,
    api_key: Option<String>,
    chain: String,
    quote_token: QuoteToken,
}

/// Determines in which token prices will get denominated in.
enum QuoteToken {
    /// Prices are denominated in ETH (the default on coingecko).
    Eth,
    /// Prices are denominated in `Token`. This is useful on chains
    /// where the native token is not ETH (e.g. xDai on gnosis chain).
    Other(Token),
}

impl CoinGecko {
    /// Authorization header for CoinGecko
    const AUTHORIZATION: &'static str = "x-cg-pro-api-key";

    pub async fn new(
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
        weth: &contracts::WETH9,
    ) -> Result<Self> {
        let quote_token = match weth
            .symbol()
            .call()
            .await
            .unwrap()
            .to_ascii_lowercase()
            .as_str()
        {
            "weth" => QuoteToken::Eth,
            _ => QuoteToken::Other(weth.address()),
        };
        let chain = match chain_id {
            1 => "ethereum".to_string(),
            100 => "xdai".to_string(),
            42161 => "arbitrum-one".to_string(),
            n => anyhow::bail!("unsupported network {n}"),
        };
        Ok(Self {
            client,
            base_url,
            api_key,
            chain,
            quote_token,
        })
    }

    async fn bulk_fetch_denominated_in_eth(
        &self,
        tokens: &[&Token],
    ) -> Result<HashMap<Token, f64>, PriceEstimationError> {
        let mut url = crate::url::join(&self.base_url, &self.chain);
        url.query_pairs_mut()
            .append_pair(
                "contract_addresses",
                &tokens
                    .iter()
                    .map(|token| format!("{:#x}", token))
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .append_pair("vs_currencies", "eth")
            .append_pair("precision", "full");

        let mut builder = self.client.get(url.clone());
        if let Some(ref api_key) = self.api_key {
            builder = builder.header(Self::AUTHORIZATION, api_key)
        }
        observe::coingecko_request(&url);
        let response = builder
            .send()
            .await
            .context("failed to sent CoinGecko price request")?;
        if !response.status().is_success() {
            return match response.status() {
                StatusCode::TOO_MANY_REQUESTS => Err(PriceEstimationError::RateLimited),
                status => Err(PriceEstimationError::EstimatorInternal(anyhow!(format!(
                    "CoinGecko returned non-success status code: {status}"
                )))),
            };
        }
        let response = response.text().await;
        observe::coingecko_response(&url, response.as_deref());
        let response = response.context("failed to fetch response body")?;

        let parsed_response = serde_json::from_str::<Response>(&response)
            .with_context(|| format!("failed to parse response: {response:?}"))?
            .0
            .into_iter()
            .map(|(token, price)| (token, price.eth))
            .collect();

        Ok(parsed_response)
    }

    /// Fetches the prices of the given tokens denominated in the given token.
    async fn bulk_fetch_denominated_in_token(
        &self,
        mut tokens: Vec<&Token>,
        denominator: Token,
    ) -> Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError> {
        tokens.push(&denominator);
        let prices_in_eth = self.bulk_fetch_denominated_in_eth(&tokens).await?;

        // fetch price of token we want to denominate all other prices in
        let denominator_price: Decimal = prices_in_eth
            .get(&denominator)
            .cloned()
            .ok_or(PriceEstimationError::NoLiquidity)?
            .try_into()
            .context("failed to convert price to decimal")?;

        let prices_in_denominator = tokens
            .into_iter()
            .map(|token| {
                let result = Self::denominate_price(token, denominator_price, &prices_in_eth);
                (*token, result)
            })
            .collect();

        Ok(prices_in_denominator)
    }

    /// CoinGecko provides all prices denominated in ETH.
    /// This function converts such a token price to a price denominated
    /// in a token provided by the caller.
    fn denominate_price(
        token: &Token,
        denominator_price_eth: Decimal,
        prices: &HashMap<Token, f64>,
    ) -> NativePriceEstimateResult {
        let token_price_eth: Decimal = prices
            .get(token)
            .cloned()
            .ok_or(PriceEstimationError::NoLiquidity)?
            .try_into()
            .context("failed to convert price to decimal")?;

        Ok(token_price_eth
            .checked_div(denominator_price_eth)
            .context("division by zero")?
            .to_f64()
            .context("failed to convert price back to f64")?)
    }
}

impl NativePriceBatchFetching for CoinGecko {
    fn fetch_native_prices<'a, 'b>(
        &'a self,
        requested_tokens: &'b HashSet<H160>,
    ) -> BoxFuture<'a, Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError>>
    where
        'b: 'a,
    {
        async {
            let mut tokens = requested_tokens.iter().collect::<Vec<_>>();
            match self.quote_token {
                QuoteToken::Eth => {
                    let prices = self.bulk_fetch_denominated_in_eth(&tokens).await?;
                    Ok(tokens
                        .into_iter()
                        .map(|token| {
                            let result = prices
                                .get(token)
                                .cloned()
                                .ok_or(PriceEstimationError::NoLiquidity);
                            (*token, result)
                        })
                        .collect())
                }
                QuoteToken::Other(native_price_token) => {
                    if !requested_tokens.contains(&native_price_token) {
                        tokens.push(&native_price_token);
                    }

                    self.bulk_fetch_denominated_in_token(tokens, native_price_token)
                        .await
                }
            }
        }
        .boxed()
    }

    fn max_batch_size(&self) -> usize {
        /// maximum number of price the coingecko API returns in a single batch
        const MAX_BATCH_SIZE: usize = 20;

        match self.quote_token {
            QuoteToken::Eth => MAX_BATCH_SIZE,
            // when fetching price denominated in a custom token we need to
            // fetch the price for that token in addition to the requested
            // tokens so we reserve 1 spot in the batch
            QuoteToken::Other(_) => MAX_BATCH_SIZE - 1,
        }
    }
}

impl NativePriceEstimating for CoinGecko {
    fn estimate_native_price(&self, token: Token) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let prices = self.fetch_native_prices(&HashSet::from([token])).await?;
            prices
                .get(&token)
                .ok_or(PriceEstimationError::NoLiquidity)?
                .clone()
        }
        .boxed()
    }
}

impl From<anyhow::Error> for PriceEstimationError {
    fn from(err: anyhow::Error) -> PriceEstimationError {
        PriceEstimationError::EstimatorInternal(err)
    }
}

mod observe {
    use url::Url;

    /// Observe a request to be sent to CoinGecko
    pub(super) fn coingecko_request(endpoint: &Url) {
        tracing::trace!(%endpoint, "sending request to CoinGecko");
    }

    /// Observe that a response was received from CoinGecko
    pub(super) fn coingecko_response(endpoint: &Url, res: Result<&str, &reqwest::Error>) {
        match res {
            Ok(res) => {
                tracing::trace!(%endpoint, ?res, "received response from CoinGecko")
            }
            Err(err) => {
                tracing::warn!(%endpoint, ?err, "failed to receive response from CoinGecko")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::env};

    impl CoinGecko {
        fn new_for_test(
            client: Client,
            base_url: Url,
            api_key: Option<String>,
            chain_id: u64,
        ) -> Result<Self> {
            let wxdai = addr!("e91d153e0b41518a2ce8dd3d7944fa863463a97d");
            let (chain, quote_token) = match chain_id {
                1 => ("ethereum".to_string(), QuoteToken::Eth),
                100 => ("xdai".to_string(), QuoteToken::Other(wxdai)),
                42161 => ("arbitrum-one".to_string(), QuoteToken::Eth),
                n => anyhow::bail!("unsupported network {n}"),
            };
            Ok(Self {
                client,
                base_url,
                api_key,
                chain,
                quote_token,
            })
        }
    }

    // It is ok to call this API without an API for local testing purposes as it is
    // difficulty to hit the rate limit manually
    const BASE_API_URL: &str = "https://api.coingecko.com/api/v3/simple/token_price";

    // We also need to test the PRO API, because batch requests aren't available in
    // the free version
    const BASE_API_PRO_URL: &str = "https://pro-api.coingecko.com/api/v3/simple/token_price";

    #[tokio::test]
    #[ignore]
    async fn works() {
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_URL).unwrap(),
            None,
            1,
        )
        .unwrap();

        let estimated_price = instance.estimate_native_price(native_token).await.unwrap();
        // Since the WETH precise price against ETH is not always exact to 1.0 (it can
        // vary slightly)
        assert!((0.95..=1.05).contains(&estimated_price));
    }

    #[tokio::test]
    #[ignore]
    async fn works_xdai() {
        // USDT
        let native_token = addr!("4ECaBa5870353805a9F068101A40E0f32ed605C6");
        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_PRO_URL).unwrap(),
            env::var("COIN_GECKO_API_KEY").ok(),
            100,
        )
        .unwrap();

        let estimated_price = instance.estimate_native_price(native_token).await.unwrap();
        // Since the USDT precise price against XDAI is not always exact to 1.0
        // (it can vary slightly)
        assert!((0.95..=1.05).contains(&estimated_price));
    }

    #[tokio::test]
    #[ignore]
    async fn works_multiple_tokens() {
        let usdt_token = addr!("4ECaBa5870353805a9F068101A40E0f32ed605C6");
        let usdc_token = addr!("2a22f9c3b484c3629090FeED35F17Ff8F88f76F0");
        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_PRO_URL).unwrap(),
            env::var("COIN_GECKO_API_KEY").ok(),
            100,
        )
        .unwrap();

        let estimated_price = instance
            .fetch_native_prices(&HashSet::from([usdt_token, usdc_token]))
            .await
            .unwrap();
        let usdt_price = estimated_price.get(&usdt_token).unwrap().clone();
        let usdc_price = estimated_price.get(&usdc_token).unwrap().clone();
        // Since the USDT/USDC precise price against XDAI is not always exact to
        // 1.0 (it can vary slightly)
        assert!((0.95..=1.05).contains(&usdt_price.unwrap()));
        assert!((0.95..=1.05).contains(&usdc_price.unwrap()));
    }
}
