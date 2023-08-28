use {
    super::TokenOwnerProposing,
    crate::rate_limiter::{back_off, RateLimiter, RateLimitingStrategy},
    anyhow::{ensure, Result},
    ethcontract::H160,
    prometheus::IntCounterVec,
    prometheus_metric_storage::MetricStorage,
    reqwest::{Client, StatusCode, Url},
    serde::Deserialize,
};

const BASE: &str = "https://api.ethplorer.io";
const FREE_API_KEY: &str = "freekey";

pub struct EthplorerTokenOwnerFinder {
    client: Client,
    base: Url,
    api_key: String,

    /// The low tiers for Ethplorer have very aggressive rate limiting, so be
    /// sure to setup a rate limiter for Ethplorer requests.
    rate_limiter: Option<RateLimiter>,

    metrics: &'static Metrics,
}

impl EthplorerTokenOwnerFinder {
    pub fn try_with_network(
        client: Client,
        api_key: Option<String>,
        chain_id: u64,
    ) -> Result<Self> {
        ensure!(chain_id == 1, "Ethplorer API unsupported network");
        Ok(Self {
            client,
            base: Url::try_from(BASE).unwrap(),
            api_key: api_key.unwrap_or_else(|| FREE_API_KEY.to_owned()),
            rate_limiter: None,
            metrics: Metrics::instance(observe::metrics::get_storage_registry())?,
        })
    }

    pub fn with_base_url(&mut self, base_url: Url) -> &mut Self {
        self.base = base_url;
        self
    }

    pub fn with_rate_limiter(&mut self, strategy: RateLimitingStrategy) -> &mut Self {
        self.rate_limiter = Some(RateLimiter::from_strategy(strategy, "ethplorer".to_owned()));
        self
    }

    async fn query_owners(&self, token: H160) -> Result<Vec<H160>> {
        let mut url = crate::url::join(&self.base, &format!("getTopTokenHolders/{token:?}"));
        // We technically only need one candidate, returning the top 2 in case there
        // is a race condition and tokens have just been transferred out.
        url.query_pairs_mut().append_pair("limit", "2");

        tracing::debug!(%url, "querying Ethplorer");
        // Don't log the API key!
        url.query_pairs_mut().append_pair("apiKey", &self.api_key);

        let request = self.client.get(url).send();
        let response = match &self.rate_limiter {
            Some(limiter) => limiter.execute(request, back_off::on_http_429).await??,
            _ => request.await?,
        };

        let status = response.status();
        let status_result = response.error_for_status_ref().map(|_| ());
        let body = response.text().await?;

        tracing::debug!(%status, %body, "response from Ethplorer API");

        // We need some special handling for "not a token contract" errors. In
        // this case, we just want to return an empty token holder list to conform
        // to the expectations of the `TokenHolderProposing` trait.
        if status == StatusCode::BAD_REQUEST {
            let err = serde_json::from_str::<Error>(&body)?;
            if err.not_token_contract() {
                return Ok(Default::default());
            }
        }
        status_result?;

        let parsed = serde_json::from_str::<Response>(&body)?;

        Ok(parsed
            .holders
            .into_iter()
            .map(|holder| holder.address)
            .collect())
    }
}

#[derive(Deserialize)]
struct Response {
    holders: Vec<Holder>,
}

#[derive(Deserialize)]
struct Holder {
    address: H160,
}

#[derive(Deserialize)]
struct Error {
    error: ErrorData,
}

#[derive(Deserialize)]
struct ErrorData {
    code: i64,
}

impl Error {
    fn not_token_contract(&self) -> bool {
        // https://github.com/EverexIO/Ethplorer/wiki/Ethplorer-API#error-codes
        self.error.code == 150
    }
}

#[derive(MetricStorage, Clone, Debug)]
#[metric(subsystem = "ethplorer_token_owner_finding")]
struct Metrics {
    /// Tracks number of "ok" or "err" responses from ethplorer.
    #[metric(labels("result"))]
    results: IntCounterVec,
}

#[async_trait::async_trait]
impl TokenOwnerProposing for EthplorerTokenOwnerFinder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        let metric = &self.metrics.results;
        let result = self.query_owners(token).await;
        match &result {
            Ok(_) => metric.with_label_values(&["ok"]).inc(),
            Err(err) => {
                tracing::warn!(?err, "error finding token owners with Ethplorer");
                metric.with_label_values(&["err"]).inc();
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[tokio::test]
    #[ignore]
    async fn token_finding_mainnet() {
        let finder =
            EthplorerTokenOwnerFinder::try_with_network(Client::default(), None, 1).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("1337BedC9D22ecbe766dF105c9623922A27963EC")))
            .await;
        assert!(!owners.unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn returns_no_owners_on_invalid_token() {
        let finder =
            EthplorerTokenOwnerFinder::try_with_network(Client::default(), None, 1).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("000000000000000000000000000000000000def1")))
            .await;
        assert!(owners.unwrap().is_empty());
    }
}
