use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::{
        price_estimation::{buffered::NativePriceBatchFetching, PriceEstimationError},
        token_info::{TokenInfo, TokenInfoFetching},
    },
    anyhow::{anyhow, Context, Result},
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{Client, StatusCode},
    rust_decimal::{prelude::ToPrimitive, Decimal, MathematicalOps},
    serde::Deserialize,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    url::Url,
};

#[derive(Debug, Deserialize)]
struct Response(HashMap<H160, Price>);

#[derive(Debug, Deserialize)]
struct Price {
    eth: Option<f64>,
}

type Token = H160;

pub struct CoinGecko {
    client: Client,
    base_url: Url,
    api_key: Option<String>,
    chain: String,
    denominator: Denominator,
    infos: Arc<dyn TokenInfoFetching>,
}

/// The token in which prices are denominated in.
struct Denominator {
    address: H160,
    /// Number of decimals of the token. This is necessary
    /// to know in order to normalize prices for tokens
    /// with a different number of decimals.
    decimals: u8,
}

impl CoinGecko {
    /// Authorization header for CoinGecko
    const AUTHORIZATION: &'static str = "x-cg-pro-api-key";

    pub async fn new(
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
        native_token: H160,
        token_infos: Arc<dyn TokenInfoFetching>,
    ) -> Result<Self> {
        let denominator_decimals = token_infos
            .get_token_info(native_token)
            .await?
            .decimals
            .context("could not determine decimals of native token")?;

        let denominator = Denominator {
            address: native_token,
            decimals: denominator_decimals,
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
            denominator,
            infos: token_infos,
        })
    }

    async fn bulk_fetch_denominated_in_eth(
        &self,
        tokens: &HashSet<Token>,
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
            .filter_map(|(token, price)| Some((token, price.eth?)))
            .collect();

        Ok(parsed_response)
    }

    /// Fetches the prices of the given tokens denominated in the given token.
    async fn bulk_fetch_denominated_in_token(
        &self,
        mut tokens: HashSet<Token>,
    ) -> Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError> {
        tokens.insert(self.denominator.address);

        let tokens_vec: Vec<_> = tokens.iter().cloned().collect();

        let (prices_in_eth, infos) = tokio::try_join!(
            self.bulk_fetch_denominated_in_eth(&tokens),
            self.infos.get_token_infos(&tokens_vec).map(Result::Ok),
        )?;

        // fetch price of token we want to denominate all other prices in
        let denominator_price: Decimal = prices_in_eth
            .get(&self.denominator.address)
            .cloned()
            .ok_or(PriceEstimationError::NoLiquidity)?
            .try_into()
            .context("failed to convert price to decimal")?;

        let prices_in_denominator = tokens
            .into_iter()
            .map(|token| {
                let result = Self::denominate_price(
                    &token,
                    denominator_price,
                    &prices_in_eth,
                    &infos,
                    &self.denominator,
                );
                (token, result)
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
        infos: &HashMap<Token, TokenInfo>,
        denominator: &Denominator,
    ) -> NativePriceEstimateResult {
        let token_decimals = infos
            .get(token)
            .and_then(|i| i.decimals)
            .with_context(|| format!("missing decimals: {token:?}"))?;

        let token_price_eth: Decimal = prices
            .get(token)
            .cloned()
            .ok_or(PriceEstimationError::NoLiquidity)?
            .try_into()
            .context("failed to convert price to decimal")?;

        // When the quoted token and the denominator have different number of decimals
        // the computed price effectively needs to be shifted by the difference.
        let adjustment =
            Decimal::new(10, 0).powi(denominator.decimals as i64 - token_decimals as i64);

        let price_denominated_in_token = token_price_eth
            .checked_div(denominator_price_eth)
            .context("division by zero")?
            .checked_mul(adjustment)
            .context("overflow of decimal")?
            .to_f64()
            .context("failed to convert price back to f64")?;

        Ok(price_denominated_in_token)
    }
}

impl NativePriceBatchFetching for CoinGecko {
    fn fetch_native_prices(
        &'_ self,
        tokens: HashSet<Token>,
    ) -> BoxFuture<'_, Result<HashMap<H160, NativePriceEstimateResult>, PriceEstimationError>> {
        self.bulk_fetch_denominated_in_token(tokens).boxed()
    }

    fn max_batch_size(&self) -> usize {
        /// maximum number of price the coingecko API returns in a single batch
        const MAX_BATCH_SIZE: usize = 20;

        // The estimator denominates prices in a provided token. This token
        // gets added to every batch call so we have to reserve 1 spot for it.
        MAX_BATCH_SIZE - 1
    }
}

impl NativePriceEstimating for CoinGecko {
    fn estimate_native_price(&self, token: Token) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let prices = self.fetch_native_prices(HashSet::from([token])).await?;
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
    use {
        super::*,
        crate::token_info::{MockTokenInfoFetching, TokenInfo},
        std::env,
    };

    impl CoinGecko {
        fn new_for_test(
            client: Client,
            base_url: Url,
            api_key: Option<String>,
            chain_id: u64,
            token_infos: Arc<dyn TokenInfoFetching>,
        ) -> Result<Self> {
            let (chain, denominator) = match chain_id {
                1 => (
                    "ethereum".to_string(),
                    Denominator {
                        address: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                        decimals: 18,
                    },
                ),
                100 => (
                    "xdai".to_string(),
                    Denominator {
                        address: addr!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
                        decimals: 18,
                    },
                ),
                42161 => (
                    "arbitrum-one".to_string(),
                    Denominator {
                        address: addr!("82af49447d8a07e3bd95bd0d56f35241523fbab1"),
                        decimals: 18,
                    },
                ),
                n => anyhow::bail!("unsupported network {n}"),
            };
            Ok(Self {
                client,
                base_url,
                api_key,
                chain,
                denominator,
                infos: token_infos,
            })
        }
    }

    // It is ok to call this API without an API for local testing purposes as it is
    // difficulty to hit the rate limit manually
    const BASE_API_URL: &str = "https://api.coingecko.com/api/v3/simple/token_price";

    // We also need to test the PRO API, because batch requests aren't available in
    // the free version
    const BASE_API_PRO_URL: &str = "https://pro-api.coingecko.com/api/v3/simple/token_price";

    fn default_token_info_fetcher() -> Arc<dyn TokenInfoFetching> {
        let mut mock = MockTokenInfoFetching::new();
        mock.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|t| {
                    let info = TokenInfo {
                        // all tests that don't specifically
                        // test price normalization
                        // use tokens with 18 decimals
                        decimals: Some(18),
                        symbol: None,
                    };
                    (*t, info)
                })
                .collect()
        });

        Arc::new(mock)
    }

    #[tokio::test]
    #[ignore]
    async fn works() {
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_URL).unwrap(),
            None,
            1,
            default_token_info_fetcher(),
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
            default_token_info_fetcher(),
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
            default_token_info_fetcher(),
        )
        .unwrap();

        let estimated_price = instance
            .fetch_native_prices(HashSet::from([usdt_token, usdc_token]))
            .await
            .unwrap();
        let usdt_price = estimated_price.get(&usdt_token).unwrap().clone();
        let usdc_price = estimated_price.get(&usdc_token).unwrap().clone();
        // Since the USDT/USDC precise price against XDAI is not always exact to
        // 1.0 (it can vary slightly)
        assert!((0.95..=1.05).contains(&usdt_price.unwrap()));
        assert!((0.95..=1.05).contains(&usdc_price.unwrap()));
    }

    #[tokio::test]
    #[ignore]
    async fn unknown_token_does_not_ruin_batch() {
        let usdc = addr!("2a22f9c3b484c3629090FeED35F17Ff8F88f76F0");
        let unknown_token = addr!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_PRO_URL).unwrap(),
            env::var("COIN_GECKO_API_KEY").ok(),
            100,
            default_token_info_fetcher(),
        )
        .unwrap();

        let estimated_price = instance
            .fetch_native_prices(HashSet::from([usdc, unknown_token]))
            .await
            .unwrap();
        let usdc_price = estimated_price.get(&usdc).unwrap().clone();
        let nonsense_price = estimated_price.get(&unknown_token).unwrap().clone();
        // Since the USDC precise price against XDAI is not always exact to
        // 1.0 (it can vary slightly)
        assert!((0.95..=1.05).contains(&usdc_price.unwrap()));
        assert_eq!(nonsense_price, Err(PriceEstimationError::NoLiquidity));
    }

    #[tokio::test]
    #[ignore]
    async fn prices_adjusted_for_token_decimals() {
        let usdc = addr!("2a22f9c3b484c3629090FeED35F17Ff8F88f76F0");
        let wxdai = addr!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d");
        let mut mock = MockTokenInfoFetching::new();
        mock.expect_get_token_infos().returning(move |tokens| {
            tokens
                .iter()
                .map(|t| {
                    let decimals = if *t == usdc { Some(6) } else { Some(18) };
                    let info = TokenInfo {
                        decimals,
                        symbol: None,
                    };
                    (*t, info)
                })
                .collect()
        });

        let instance = CoinGecko::new_for_test(
            Client::default(),
            Url::parse(BASE_API_PRO_URL).unwrap(),
            env::var("COIN_GECKO_API_KEY").ok(),
            100,
            Arc::new(mock),
        )
        .unwrap();

        // usdc_price should be: ~1000000000000.0
        let usdc_price = instance.estimate_native_price(usdc).await.unwrap();
        // wxdai_price should be: ~1.0
        let wxdai_price = instance.estimate_native_price(wxdai).await.unwrap();
        dbg!(usdc_price, wxdai_price);

        // The `USDC` token only has 6 decimals whereas `wxDai` has 18. To make
        // the prices comparable we therefor have to shift `usdc_price` 12 decimals
        // to the right.
        let usdc_price_adjusted = usdc_price / 10f64.powi(12);

        // Since Dai and USDC both track the US dollar they should at least be
        // within 5% of each other after adjusting for their respective decimals.
        assert!((wxdai_price * 0.95..=wxdai_price * 1.05).contains(&usdc_price_adjusted));
        assert!((0.95..=1.05).contains(&wxdai_price))
    }
}
