use {
    super::TokenOwnerProposing,
    anyhow::Result,
    chain::Chain,
    ethcontract::H160,
    prometheus::IntCounterVec,
    prometheus_metric_storage::MetricStorage,
    rate_limit::{back_off, RateLimiter, Strategy},
    reqwest::{Client, Url},
    serde::Deserialize,
};

pub struct BlockscoutTokenOwnerFinder {
    client: Client,
    base: Url,
    api_key: Option<String>,
    rate_limiter: Option<RateLimiter>,
}

impl BlockscoutTokenOwnerFinder {
    pub fn with_network(client: Client, chain: &Chain) -> Result<Self> {
        let base_url = match chain {
            Chain::Mainnet => "https://eth.blockscout.com/api",
            Chain::Goerli => "https://eth-goerli.blockscout.com/api",
            Chain::Gnosis => "https://blockscout.com/xdai/mainnet/api",
            Chain::Sepolia => "https://eth-sepolia.blockscout.com/api",
            Chain::ArbitrumOne => "https://arbitrum.blockscout.com/api",
            Chain::Base => "https://base.blockscout.com/api",
            Chain::Hardhat => anyhow::bail!("Hardhat chain not supported"),
        };

        Ok(Self {
            client,
            base: Url::parse(base_url)?,
            api_key: None,
            rate_limiter: None,
        })
    }

    pub fn with_base_url(&mut self, base_url: Url) -> &mut Self {
        self.base = base_url;
        self
    }

    pub fn with_api_key(&mut self, api_key: String) -> &mut Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_rate_limiter(&mut self, strategy: Strategy) -> &mut Self {
        self.rate_limiter = Some(RateLimiter::from_strategy(
            strategy,
            "blockscout".to_owned(),
        ));
        self
    }

    async fn query_owners(&self, token: H160) -> Result<Vec<H160>> {
        let mut url = self.base.clone();
        url.query_pairs_mut()
            .append_pair("module", "token")
            .append_pair("action", "getTokenHolders")
            .append_pair("contractaddress", &format!("{token:#x}"));

        // Don't log the API key!
        tracing::debug!(%url, "Querying Blockscout API");

        if let Some(api_key) = &self.api_key {
            url.query_pairs_mut().append_pair("apikey", api_key);
        }

        let request = self.client.get(url).send();
        let response = match &self.rate_limiter {
            Some(limiter) => limiter.execute(request, back_off::on_http_429).await??,
            _ => request.await?,
        };
        let status = response.status();
        let status_result = response.error_for_status_ref().map(|_| ());
        let body = response.text().await?;

        tracing::debug!(%status, %body, "Response from Blockscout API");

        status_result?;
        let parsed = serde_json::from_str::<Response>(&body)?;

        // We technically only need one candidate, returning the top 2 in case there is
        // a race condition and tokens have just been transferred out
        Ok(parsed
            .result
            .into_iter()
            .map(|owner| owner.address)
            .take(2)
            .collect())
    }
}

#[derive(Deserialize)]
struct Response {
    result: Vec<TokenOwner>,
}

#[derive(Deserialize)]
struct TokenOwner {
    address: H160,
}

#[derive(MetricStorage, Clone, Debug)]
#[metric(subsystem = "blockscout_token_owner_finding")]
struct Metrics {
    /// Tracks number of "ok" or "err" responses from blockscout.
    #[metric(labels("result"))]
    results: IntCounterVec,
}

#[async_trait::async_trait]
impl TokenOwnerProposing for BlockscoutTokenOwnerFinder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        let metric = &Metrics::instance(observe::metrics::get_storage_registry())
            .unwrap()
            .results;

        match self.query_owners(token).await {
            Ok(ok) => {
                metric.with_label_values(&["ok"]).inc();
                Ok(ok)
            }
            Err(err) => {
                tracing::warn!(?err, "error finding token owners with Blockscout");
                metric.with_label_values(&["err"]).inc();
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_mainnet() {
        let finder =
            BlockscoutTokenOwnerFinder::with_network(Client::default(), &Chain::Mainnet).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("1337BedC9D22ecbe766dF105c9623922A27963EC")))
            .await;
        assert!(!owners.unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_xdai() {
        let finder =
            BlockscoutTokenOwnerFinder::with_network(Client::default(), &Chain::Gnosis).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("1337BedC9D22ecbe766dF105c9623922A27963EC")))
            .await;
        assert!(!owners.unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_no_owners() {
        let finder =
            BlockscoutTokenOwnerFinder::with_network(Client::default(), &Chain::Gnosis).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("000000000000000000000000000000000000def1")))
            .await;
        assert!(owners.unwrap().is_empty());
    }
}
