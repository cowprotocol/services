use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{buffered::NativePriceBatchFetcher, PriceEstimationError},
    anyhow::{anyhow, Result},
    async_trait::async_trait,
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{Client, StatusCode},
    rust_decimal::prelude::ToPrimitive,
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

    pub async fn send_bulk_request_price_in_eth(
        &self,
        tokens: &[&Token],
    ) -> Result<HashMap<Token, NativePriceEstimateResult>, PriceEstimationError> {
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
            let status = response.status();
            return match status {
                StatusCode::TOO_MANY_REQUESTS => {
                    return Ok(tokens
                        .iter()
                        .map(|token| (**token, Err(PriceEstimationError::RateLimited)))
                        .collect());
                }
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
        let prices = serde_json::from_str::<Response>(&response)
            .map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to parse native CoinGecko prices from {response:?}: {e:?}"
                ))
            })?
            .0;

        Ok(tokens
            .iter()
            .map(|token| {
                (
                    **token,
                    prices
                        .get(token)
                        .ok_or(PriceEstimationError::NoLiquidity)
                        .map(|price| price.eth),
                )
            })
            .collect())
    }

    pub async fn send_request_price_in_eth(&self, token: Token) -> NativePriceEstimateResult {
        let prices = self.send_bulk_request_price_in_eth(&[&token]).await?;
        prices
            .get(&token)
            .ok_or(PriceEstimationError::NoLiquidity)?
            .clone()
    }
}

#[async_trait]
impl NativePriceBatchFetcher for CoinGecko {
    async fn fetch_native_prices(
        &self,
        tokens: &HashSet<H160>,
    ) -> std::result::Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError> {
        let mut requested_tokens = tokens.iter().collect::<Vec<_>>();
        match self.quote_token {
            QuoteToken::Eth => self.send_bulk_request_price_in_eth(&requested_tokens).await,
            QuoteToken::Other(native_price_token) => {
                requested_tokens.push(&native_price_token);
                let prices = self
                    .send_bulk_request_price_in_eth(&requested_tokens)
                    .await?;
                let native_price_token = prices
                    .get(&native_price_token)
                    .ok_or(PriceEstimationError::NoLiquidity)?
                    .clone();
                let native_price_token_eth = rust_decimal::Decimal::try_from(native_price_token?)
                    .map_err(|e| {
                    PriceEstimationError::EstimatorInternal(anyhow!(
                        "failed to parse native price token in ETH to rust decimal: {e:?}"
                    ))
                })?;
                prices
                    .into_iter()
                    .filter(|(token, _)| tokens.contains(token))
                    .map(|(token, price)| match price.as_ref() {
                        Ok(price) => {
                            let token_eth =
                                rust_decimal::Decimal::try_from(*price).map_err(|e| {
                                    PriceEstimationError::EstimatorInternal(anyhow!(
                                        "failed to parse requested token in ETH to rust decimal: \
                                         {e:?}"
                                    ))
                                })?;
                            let token_in_native_price = token_eth
                                .checked_div(native_price_token_eth)
                                .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                                    "division by zero"
                                )))?;
                            let token_in_native_price_f64 = token_in_native_price.to_f64().ok_or(
                                PriceEstimationError::EstimatorInternal(anyhow!(
                                    "failed to parse result to f64"
                                )),
                            )?;
                            Ok((token, Ok(token_in_native_price_f64)))
                        }
                        Err(_) => Ok((token, price)),
                    })
                    .collect::<Result<_, _>>()
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
    pub fn coingecko_request(endpoint: &Url) {
        tracing::trace!(%endpoint, "sending request to CoinGecko");
    }

    /// Observe that a response was received from CoinGecko
    pub fn coingecko_response(endpoint: &Url, res: Result<&str, &reqwest::Error>) {
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
