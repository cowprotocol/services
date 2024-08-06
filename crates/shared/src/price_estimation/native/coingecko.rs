use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    anyhow::{anyhow, Result},
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{Client, StatusCode},
    rust_decimal::prelude::ToPrimitive,
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
    native_price: NativePrice,
}

enum NativePrice {
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
        let native_price = match weth
            .symbol()
            .call()
            .await
            .unwrap()
            .to_ascii_lowercase()
            .as_str()
        {
            "weth" => NativePrice::Eth,
            _ => NativePrice::Other(weth.address()),
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
            native_price,
        })
    }

    pub async fn send_request_price_in_eth(&self, token: Token) -> NativePriceEstimateResult {
        let mut url = crate::url::join(&self.base_url, &self.chain);
        url.query_pairs_mut()
            .append_pair("contract_addresses", &format!("{:#x}", token))
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
}

impl NativePriceEstimating for CoinGecko {
    fn estimate_native_price(&self, token: Token) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            match self.native_price {
                NativePrice::Eth => self.send_request_price_in_eth(token).await,
                NativePrice::Other(native_price_token) => {
                    let token_eth = rust_decimal::Decimal::try_from(
                        self.send_request_price_in_eth(token).await?,
                    )
                    .map_err(|e| {
                        PriceEstimationError::EstimatorInternal(anyhow!(
                            "failed to parse requested token in ETH to rust decimal: {e:?}"
                        ))
                    })?;
                    let native_price_token_eth = rust_decimal::Decimal::try_from(
                        self.send_request_price_in_eth(native_price_token).await?,
                    )
                    .map_err(|e| {
                        PriceEstimationError::EstimatorInternal(anyhow!(
                            "failed to parse native price token in ETH to rust decimal: {e:?}"
                        ))
                    })?;
                    let token_in_native_price =
                        token_eth.checked_div(native_price_token_eth).ok_or(
                            PriceEstimationError::EstimatorInternal(anyhow!("division by zero")),
                        )?;
                    token_in_native_price
                        .to_f64()
                        .ok_or(PriceEstimationError::EstimatorInternal(anyhow!(
                            "failed to parse result to f64"
                        )))
                }
            }
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
