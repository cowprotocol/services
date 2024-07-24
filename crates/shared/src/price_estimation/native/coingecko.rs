use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    anyhow::{anyhow, Result},
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{header::AUTHORIZATION, Client, StatusCode},
    serde::Deserialize,
    std::collections::HashMap,
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
}

impl CoinGecko {
    pub fn new(
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
    ) -> Result<Self> {
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
        })
    }
}

impl NativePriceEstimating for CoinGecko {
    fn estimate_native_price(&self, token: Token) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let url = format!(
                "{}/{}?contract_addresses={token:#x}&vs_currencies=eth",
                self.base_url, self.chain
            );
            let mut builder = self.client.get(&url);
            if let Some(ref api_key) = self.api_key {
                builder = builder.header(AUTHORIZATION, api_key)
            }
            observe::coingecko_request(&url);
            let response = builder.send().await;
            observe::coingecko_response(&url, response.as_ref());
            let response = response.map_err(|e| {
                PriceEstimationError::EstimatorInternal(anyhow!(
                    "failed to sent CoinGecko price request: {e:?}"
                ))
            })?;
            if !response.status().is_success() {
                let status = response.status();
                return match status {
                    StatusCode::TOO_MANY_REQUESTS => Err(PriceEstimationError::RateLimited),
                    status => Err(PriceEstimationError::EstimatorInternal(anyhow!(
                        "failed to retrieve prices from CoinGecko: error with status code \
                         {status}."
                    ))),
                };
            }
            let response = response.text().await.map_err(|e| {
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

            let price = prices
                .get(&token)
                .ok_or(PriceEstimationError::NoLiquidity)?;
            Ok(price.eth)
        }
        .boxed()
    }
}

mod observe {
    use reqwest::Response;

    /// Observe a request to be sent to CoinGecko
    pub fn coingecko_request(endpoint: &str) {
        tracing::trace!(%endpoint, "sending request to CoinGecko");
    }

    /// Observe that a response was received from CoinGecko
    pub fn coingecko_response(endpoint: &str, res: Result<&Response, &reqwest::Error>) {
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
    use {super::*, std::str::FromStr};

    // It is ok to call this API without an API for local testing purposes as it is
    // difficulty to hit the rate limit manually
    const BASE_URL: &str = "https://api.coingecko.com/api/v3/simple/token_price";

    #[tokio::test]
    #[ignore]
    async fn works() {
        let native_token = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        let instance =
            CoinGecko::new(Client::default(), Url::parse(BASE_URL).unwrap(), None, 1).unwrap();

        let estimated_price = instance.estimate_native_price(native_token).await.unwrap();
        // Since the WETH precise price against ETH is not always exact to 1.0 (it can
        // vary slightly)
        assert!((0.95..=1.05).contains(&estimated_price));
    }
}
