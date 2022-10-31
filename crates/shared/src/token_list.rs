use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use ethcontract::H160;
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone, Debug, Default)]
pub struct TokenListConfiguration {
    pub url: String,
    pub chain_id: u64,
    pub client: Client,
    pub update_interval: Duration,
}

impl TokenListConfiguration {
    async fn tokens(&self) -> Result<HashMap<H160, Token>> {
        let model: TokenListModel = self
            .client
            .get(self.url.clone())
            .send()
            .await?
            .json()
            .await?;
        Ok(Self::from_tokens(model.tokens, self.chain_id))
    }

    fn from_tokens(tokens: Vec<TokenModel>, chain_id: u64) -> HashMap<H160, Token> {
        tokens
            .into_iter()
            .filter(|token| token.chain_id == chain_id)
            .map(|token| (token.token.address, token.token))
            .collect()
    }
}
#[derive(Clone, Debug, Default)]
pub struct TokenList {
    tokens: Arc<RwLock<HashMap<H160, Token>>>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub address: H160,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

impl TokenList {
    pub async fn from_configuration(configuration: TokenListConfiguration) -> Self {
        let tokens = Arc::new(RwLock::new(
            configuration.tokens().await.unwrap_or_default(),
        ));

        // spawn a background task to regularly update token list
        {
            let tokens = tokens.clone();
            let updater = async move {
                loop {
                    tokio::time::sleep(configuration.update_interval).await;

                    if let Ok(new_tokens) = configuration.tokens().await {
                        let mut w = tokens.write().unwrap();
                        *w = new_tokens;
                    }
                }
            };
            tokio::task::spawn(updater);
        }

        Self { tokens }
    }

    pub fn new(tokens: HashMap<H160, Token>) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(tokens)),
        }
    }

    pub fn get(&self, address: &H160) -> Option<Token> {
        self.tokens.read().unwrap().get(address).cloned()
    }

    pub fn all(&self) -> Vec<Token> {
        self.tokens.read().unwrap().values().cloned().collect()
    }

    pub fn addresses(&self) -> HashSet<H160> {
        self.tokens.read().unwrap().keys().cloned().collect()
    }
}

/// Relevant parts of TokenList schema as defined in https://uniswap.org/tokenlist.schema.json
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TokenListModel {
    name: String,
    tokens: Vec<TokenModel>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TokenModel {
    chain_id: u64,
    #[serde(flatten)]
    token: Token,
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
                        token: Token {
                            address: testlib::tokens::USDC,
                            name: "USD Coin".into(),
                            symbol: "USDC".into(),
                            decimals: 6,
                        }
                    },
                    TokenModel {
                        chain_id: 4,
                        token: Token {
                            address: addr!("39AA39c021dfbaE8faC545936693aC917d5E7563"),
                            name: "Compound USD Coin".into(),
                            symbol: "cUSDC".into(),
                            decimals: 8,
                        }
                    }
                ]
            }
        );
    }

    #[test]
    fn test_creation_with_chain_id() {
        let list = serde_json::from_str::<TokenListModel>(EXAMPLE_LIST).unwrap();
        let tokens = TokenListConfiguration::from_tokens(list.tokens, 1);
        let instance = TokenList::new(tokens);
        assert!(instance.get(&testlib::tokens::USDC).is_some());
        // Chain ID 4
        assert!(instance
            .get(&addr!("39AA39c021dfbaE8faC545936693aC917d5E7563"))
            .is_none());
    }
}
