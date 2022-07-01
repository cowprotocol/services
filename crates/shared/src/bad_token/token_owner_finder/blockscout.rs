use anyhow::{bail, Result};
use ethcontract::H160;
use prometheus::IntCounterVec;
use prometheus_metric_storage::MetricStorage;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::Duration;

use super::TokenOwnerFinding;

const BASE: &str = "https://blockscout.com/";
// Blockscout uses a custom timeout because their api is often slow. We would like those requests
// to finish even if slow as bad token detection results are cached for a while and faster
// TokenOwnerFinding implementations are not slowed down by slower ones.
const TIMEOUT: Duration = Duration::from_secs(45);

pub struct BlockscoutTokenOwnerFinder {
    client: Client,
    base: Url,
}

impl BlockscoutTokenOwnerFinder {
    pub fn try_with_network(client: Client, network_id: u64) -> Result<Self> {
        let network = match network_id {
            1 => "eth/",
            100 => "xdai/",
            _ => bail!("Unsupported Network"),
        };
        Ok(Self {
            client,
            base: Url::try_from(BASE)
                .expect("Invalid Blockscout Base URL")
                .join(network)
                .expect("Invalid Blockscout URL Segement")
                .join("mainnet/api")
                .expect("Invalid Blockscout URL Segement"),
        })
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
impl TokenOwnerFinding for BlockscoutTokenOwnerFinder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        let mut url = self.base.clone();
        url.query_pairs_mut()
            .append_pair("module", "token")
            .append_pair("action", "getTokenHolders")
            .append_pair("contractaddress", &format!("{token:#x}"));

        let metric = &Metrics::instance(global_metrics::get_metric_storage_registry())
            .unwrap()
            .results;
        tracing::debug!("Querying Blockscout API: {}", url);
        let request = self.client.get(url).timeout(TIMEOUT).send();
        let response_text = match async { request.await?.text().await }.await {
            Ok(response) => {
                metric.with_label_values(&["ok"]).inc();
                response
            }
            Err(err) => {
                metric.with_label_values(&["err"]).inc();
                return Err(err.into());
            }
        };
        tracing::debug!("Response from Blockscout API: {}", response_text);

        let parsed = serde_json::from_str::<Response>(&response_text)?;
        let mut addresses: Vec<_> = parsed
            .result
            .into_iter()
            .map(|owner| owner.address)
            .collect();
        // We technically only need one candidate, returning the top 2 in case there is a race condition and tokens have just been transferred out
        addresses.truncate(2);
        Ok(addresses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_mainnet() {
        let finder = BlockscoutTokenOwnerFinder::try_with_network(Client::default(), 1).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("1337BedC9D22ecbe766dF105c9623922A27963EC")))
            .await;
        assert!(!owners.unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_xdai() {
        let finder = BlockscoutTokenOwnerFinder::try_with_network(Client::default(), 100).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("1337BedC9D22ecbe766dF105c9623922A27963EC")))
            .await;
        assert!(!owners.unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_blockscout_token_finding_no_owners() {
        let finder = BlockscoutTokenOwnerFinder::try_with_network(Client::default(), 100).unwrap();
        let owners = finder
            .find_candidate_owners(H160(hex!("000000000000000000000000000000000000def1")))
            .await;
        assert!(owners.unwrap().is_empty());
    }
}
