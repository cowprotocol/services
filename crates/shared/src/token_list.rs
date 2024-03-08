use {
    anyhow::Result,
    ethcontract::H160,
    prometheus::IntCounterVec,
    reqwest::{Client, Url},
    serde::Deserialize,
    std::{
        collections::HashSet,
        sync::{Arc, RwLock},
        time::Duration,
    },
    tracing::Instrument,
};

#[derive(Clone, Debug, Default)]
pub struct TokenListConfiguration {
    pub url: Option<Url>,
    pub chain_id: u64,
    pub client: Client,
    pub update_interval: Duration,
    pub hardcoded: Vec<H160>,
}

impl TokenListConfiguration {
    async fn get_external_list(&self) -> Result<HashSet<H160>> {
        let model: TokenListModel = if let Some(url) = &self.url {
            self.client.get(url.clone()).send().await?.json().await?
        } else {
            Default::default()
        };
        Ok(self.get_list(model.tokens))
    }

    fn get_list(&self, tokens: Vec<TokenModel>) -> HashSet<H160> {
        tokens
            .into_iter()
            .filter(|token| token.chain_id == self.chain_id)
            .map(|token| token.address)
            .chain(self.hardcoded.iter().copied())
            .collect()
    }
}
#[derive(Clone, Debug, Default)]
pub struct AutoUpdatingTokenList {
    tokens: Arc<RwLock<HashSet<H160>>>,
}

impl AutoUpdatingTokenList {
    pub async fn from_configuration(configuration: TokenListConfiguration) -> Self {
        let tokens = Arc::new(RwLock::new(match configuration.get_external_list().await {
            Ok(tokens) => tokens,
            Err(err) => {
                tracing::error!(?err, "failed to initialize token list");
                Default::default()
            }
        }));

        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

        // spawn a background task to regularly update token list
        {
            let tokens = tokens.clone();
            let updater = async move {
                loop {
                    tokio::time::sleep(configuration.update_interval).await;

                    match configuration.get_external_list().await {
                        Ok(new_tokens) => {
                            metrics
                                .token_list_updates
                                .with_label_values(&["success"])
                                .inc();
                            let mut w = tokens.write().unwrap();
                            *w = new_tokens;
                        }
                        Err(err) => {
                            metrics
                                .token_list_updates
                                .with_label_values(&["failure"])
                                .inc();
                            tracing::warn!(?err, "failed to update token list")
                        }
                    }
                }
            };
            tokio::task::spawn(updater.instrument(tracing::info_span!("auto_updating_token_list")));
        }

        Self { tokens }
    }

    pub fn new(tokens: HashSet<H160>) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(tokens)),
        }
    }

    pub fn contains(&self, address: &H160) -> bool {
        self.tokens.read().unwrap().contains(address)
    }

    pub fn all(&self) -> HashSet<H160> {
        self.tokens.read().unwrap().clone()
    }
}

/// Relevant parts of AutoUpdatingTokenList schema as defined in https://uniswap.org/tokenlist.schema.json
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TokenListModel {
    name: String,
    tokens: Vec<TokenModel>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TokenModel {
    chain_id: u64,
    address: H160,
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
struct Metrics {
    /// Tracks how often a token list update succeeded or failed.
    #[metric(labels("result"))]
    token_list_updates: IntCounterVec,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // https://github.com/Uniswap/token-lists/blob/master/test/schema/example.tokenlist.json
    const EXAMPLE_LIST: &str = r#"
    {
        "name": "My Token List",
        "logoURI": "ipfs://QmUSNbwUxUYNMvMksKypkgWs8unSm8dX2GjCPBVGZ7GGMr",
        "keywords": [
        "audited",
        "verified",
        "special tokens"
        ],
        "tags": {
        "stablecoin": {
            "name": "Stablecoin",
            "description": "Tokens that are fixed to an external asset, e.g. the US dollar"
        },
        "compound": {
            "name": "Compound Finance",
            "description": "Tokens that earn interest on compound.finance"
        }
        },
        "timestamp": "2020-06-12T00:00:00+00:00",
        "tokens": [
        {
            "chainId": 1,
            "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "symbol": "USDC",
            "name": "USD Coin",
            "decimals": 6,
            "logoURI": "ipfs://QmXfzKRvjZz3u5JRgC4v5mGVbm9ahrUiB4DgzHBsnWbTMM",
            "tags": [
            "stablecoin"
            ]
        },
        {
            "chainId": 4,
            "address": "0x39AA39c021dfbaE8faC545936693aC917d5E7563",
            "symbol": "cUSDC",
            "name": "Compound USD Coin",
            "decimals": 8,
            "logoURI": "ipfs://QmUSNbwUxUYNMvMksKypkgWs8unSm8dX2GjCPBVGZ7GGMr",
            "tags": [
            "compound"
            ]
        }
        ],
        "version": {
        "major": 1,
        "minor": 0,
        "patch": 0
        }
    }"#;

    #[test]
    fn test_deserialization() {
        let list = serde_json::from_str::<TokenListModel>(EXAMPLE_LIST).unwrap();
        assert_eq!(
            list,
            TokenListModel {
                name: "My Token List".into(),
                tokens: vec![
                    TokenModel {
                        chain_id: 1,
                        address: testlib::tokens::USDC,
                    },
                    TokenModel {
                        chain_id: 4,
                        address: addr!("39AA39c021dfbaE8faC545936693aC917d5E7563"),
                    }
                ]
            }
        );
    }

    #[test]
    fn test_creation_with_chain_id() {
        let list = serde_json::from_str::<TokenListModel>(EXAMPLE_LIST).unwrap();
        let config = TokenListConfiguration {
            url: Default::default(),
            chain_id: 1,
            client: Default::default(),
            update_interval: Default::default(),
            hardcoded: Default::default(),
        };
        let tokens = config.get_list(list.tokens);
        let instance = AutoUpdatingTokenList::new(tokens);
        assert!(instance.contains(&testlib::tokens::USDC));
        // Chain ID 4
        assert!(!instance.contains(&addr!("39AA39c021dfbaE8faC545936693aC917d5E7563")),);
    }

    #[ignore]
    #[tokio::test]
    async fn cow_list() {
        let list = serde_json::from_str::<TokenListModel>(EXAMPLE_LIST).unwrap();
        let mut config = TokenListConfiguration {
            url: Some("https://files.cow.fi/token_list.json".parse().unwrap()),
            chain_id: 1,
            client: Default::default(),
            update_interval: Default::default(),
            hardcoded: Default::default(),
        };
        let tokens = config.get_external_list().await.unwrap();
        assert!(tokens.contains(&testlib::tokens::USDC));
        let gc_token = addr!("39AA39c021dfbaE8faC545936693aC917d5E7563");
        assert!(!tokens.contains(&gc_token));

        config.chain_id = 4;
        let tokens = config.get_list(list.tokens);
        assert!(!tokens.contains(&testlib::tokens::USDC));
        assert!(tokens.contains(&gc_token));
    }
}
