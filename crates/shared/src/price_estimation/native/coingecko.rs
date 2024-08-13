use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{buffered::NativePriceBatchFetcher, PriceEstimationError},
    anyhow::{anyhow, Result},
    async_trait::async_trait,
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

enum QuoteToken {
    Eth,
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
        let response = builder.send().await.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow!(
                "failed to sent CoinGecko price request: {e:?}"
            ))
        })?;
        if !response.status().is_success() {
            return match response.status() {
                StatusCode::TOO_MANY_REQUESTS => Err(PriceEstimationError::RateLimited),
                status => Err(PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to retrieve prices from CoinGecko: error with status code {status}."
                ))),
            };
        }
        let response = response.text().await;
        observe::coingecko_response(&url, response.as_deref());
        let response = response.map_err(|e| {
            PriceEstimationError::EstimatorInternal(anyhow!(
                "failed to fetch native CoinGecko prices: {e:?}"
            ))
        })?;

        let parsed_response = serde_json::from_str::<Response>(&response)
            .map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to parse native CoinGecko prices from {response:?}: {e:?}"
                ))
            })?
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
            .map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to parse native price token in ETH to rust decimal: {e:?}"
                ))
            })?;

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
            .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                "response did not contain price for {token:?}"
            )))?
            .try_into()
            .map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to parse requested token in ETH to rust decimal: {e:?}"
                ))
            })?;

        token_price_eth
            .checked_div(denominator_price_eth)
            .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                "division by zero"
            )))?
            .to_f64()
            .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                "failed to convert price to f64"
            )))
    }
}

#[async_trait]
impl NativePriceBatchFetcher for CoinGecko {
    async fn fetch_native_prices(
        &self,
        requested_tokens: &HashSet<H160>,
    ) -> Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError> {
        let mut tokens = requested_tokens.iter().collect::<Vec<_>>();
        match self.quote_token {
            QuoteToken::Eth => {
                let prices = self.bulk_fetch_denominated_in_eth(&tokens).await?;
                Ok(prices
                    .into_iter()
                    .map(|(token, price)| (token, Ok(price)))
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
    use {
        super::*,
        lazy_static::lazy_static,
        std::{env, str::FromStr},
    };

    lazy_static! {
        static ref WXDAI_TOKEN_ADDRESS: H160 = "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d"
            .parse()
            .unwrap();
    }

    impl CoinGecko {
        fn new_for_test(
            client: Client,
            base_url: Url,
            api_key: Option<String>,
            chain_id: u64,
        ) -> Result<Self> {
            let (chain, quote_token) = match chain_id {
                1 => ("ethereum".to_string(), QuoteToken::Eth),
                100 => ("xdai".to_string(), QuoteToken::Other(*WXDAI_TOKEN_ADDRESS)),
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
        let native_token = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
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
        let native_token = H160::from_str("0x4ECaBa5870353805a9F068101A40E0f32ed605C6").unwrap();
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
        let usdt_token = H160::from_str("0x4ECaBa5870353805a9F068101A40E0f32ed605C6").unwrap();
        let usdc_token = H160::from_str("0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0").unwrap();
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
