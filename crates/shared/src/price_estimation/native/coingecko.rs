use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    anyhow::{anyhow, Result},
    futures::{future::BoxFuture, FutureExt},
    lazy_static::lazy_static,
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

lazy_static! {
    static ref WXDAI_TOKEN_ADDRESS: H160 = "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d"
        .parse()
        .unwrap();
}

impl CoinGecko {
    /// Authorization header for CoinGecko
    const AUTHORIZATION: &'static str = "x-cg-pro-api-key";

    pub fn new(
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
    ) -> Result<Self> {
        let (chain, native_price) = match chain_id {
            1 => ("ethereum".to_string(), NativePrice::Eth),
            100 => ("xdai".to_string(), NativePrice::Other(*WXDAI_TOKEN_ADDRESS)),
            42161 => ("arbitrum-one".to_string(), NativePrice::Eth),
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

    #[tokio::test]
    #[ignore]
    async fn works_xdai() {
        // USDT
        let native_token = H160::from_str("0x4ECaBa5870353805a9F068101A40E0f32ed605C6").unwrap();
        let instance =
            CoinGecko::new(Client::default(), Url::parse(BASE_URL).unwrap(), None, 100).unwrap();

        let estimated_price = instance.estimate_native_price(native_token).await.unwrap();
        // Since the USDT precise price against XDAI is not always exact to 1.0
        // (it can vary slightly)
        assert!((0.95..=1.05).contains(&estimated_price));
    }
}
