//! Module containing The Graph API client used for retrieving Uniswap V3
//! data from the Uniswap V3 subgraph.

use std::collections::HashMap;

use crate::{
    event_handling::MAX_REORG_BLOCK_COUNT,
    subgraph::{ContainsId, SubgraphClient},
};
use anyhow::{bail, Result};
use ethcontract::{H160, U256};
use model::u256_decimal;
use num::BigInt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use serde_with::{serde_as, DisplayFromStr};

const ALL_POOLS_QUERY: &str = r#"
    query Pools($block: Int, $pageSize: Int, $lastId: ID) {
        pools(
            block: { number: $block }
            first: $pageSize
            where: {
                id_gt: $lastId
                tick_not: null
                ticks_: { liquidityNet_not: "0" }
            }
        ) {
            id
            token0 {
                symbol
                id
                decimals
            }
            token1 {
                symbol
                id
                decimals
            }
            feeTier
            liquidity
            sqrtPrice
            tick
            totalValueLockedETH
        }
    }
"#;

const POOLS_BY_IDS_QUERY: &str = r#"
    query Pools($block: Int, $pool_ids: [ID], $pageSize: Int, $lastId: ID) {
        pools(
            block: { number: $block }
            first: $pageSize
            where: {
                id_in: $pool_ids
                id_gt: $lastId
                tick_not: null
                ticks_: { liquidityNet_not: "0" }
            }
        ) {
            id
            token0 {
                symbol
                id
                decimals
            }
            token1 {
                symbol
                id
                decimals
            }
            feeTier
            liquidity
            sqrtPrice
            tick
            totalValueLockedETH
        }
    }
"#;

const TICKS_BY_POOL_IDS_QUERY: &str = r#"
    query Ticks($block: Int, $pool_ids: [ID], $pageSize: Int, $lastId: ID) {
        ticks(
            block: { number: $block }
            first: $pageSize
            where: {
                id_gt: $lastId
                liquidityNet_not: "0"
                pool_: { id_in: $pool_ids }
            }
        ) {
            id
            tickIdx
            liquidityNet
            poolAddress
        }
    }
"#;

/// A client to the Uniswap V3 subgraph.
///
/// This client is not implemented to allow general GraphQL queries, but instead
/// implements high-level methods that perform GraphQL queries under the hood.
pub struct UniV3SubgraphClient(SubgraphClient);

impl UniV3SubgraphClient {
    /// Creates a new Uniswap V3 subgraph client for the specified chain ID.
    pub fn for_chain(chain_id: u64, client: Client) -> Result<Self> {
        let subgraph_name = match chain_id {
            1 => "uniswap-v3",
            _ => bail!("unsupported chain {}", chain_id),
        };
        Ok(Self(SubgraphClient::new("uniswap", subgraph_name, client)?))
    }

    async fn get_pools(&self, query: &str, variables: Map<String, Value>) -> Result<Vec<PoolData>> {
        Ok(self
            .0
            .paginated_query(query, variables)
            .await?
            .into_iter()
            .filter(|pool: &PoolData| pool.total_value_locked_eth.is_normal())
            .collect())
    }

    /// Retrieves the pool data for all existing pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        let block_number = self.get_safe_block().await?;
        let variables = json_map! {
            "block" => block_number,
        };
        let pools = self.get_pools(ALL_POOLS_QUERY, variables).await?;
        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools,
        })
    }

    /// Retrieves the pool data for pools with given pool ids
    async fn get_pools_by_pool_ids(
        &self,
        pool_ids: &[H160],
        block_number: u64,
    ) -> Result<Vec<PoolData>> {
        let variables = json_map! {
            "block" => block_number,
            "pool_ids" => json!(pool_ids)
        };
        let pools = self.get_pools(POOLS_BY_IDS_QUERY, variables).await?;
        Ok(pools)
    }

    /// Retrieves the ticks data for pools with given pool ids
    async fn get_ticks_by_pools_ids(
        &self,
        pool_ids: &[H160],
        block_number: u64,
    ) -> Result<Vec<TickData>> {
        let variables = json_map! {
            "block" => block_number,
            "pool_ids" => json!(pool_ids)
        };
        let result = self
            .0
            .paginated_query(TICKS_BY_POOL_IDS_QUERY, variables)
            .await?;
        Ok(result)
    }

    /// Retrieves the pool data and ticks data for pools with given pool ids
    pub async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[H160],
        block_number: u64,
    ) -> Result<Vec<PoolData>> {
        let (pools, ticks) = futures::try_join!(
            self.get_pools_by_pool_ids(ids, block_number),
            self.get_ticks_by_pools_ids(ids, block_number)
        )?;

        // group ticks by pool ids
        let mut ticks_mapped = HashMap::new();
        for tick in ticks {
            ticks_mapped
                .entry(tick.pool_address)
                .or_insert(vec![])
                .push(tick);
        }

        Ok(pools
            .into_iter()
            .filter_map(|mut pool| {
                ticks_mapped.get(&pool.id).map(|ticks| {
                    pool.ticks = Some(ticks.clone());
                    pool
                })
            })
            .collect())
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    pub async fn get_safe_block(&self) -> Result<u64> {
        // Ideally we would want to use block hash here so that we can check
        // that there indeed is no reorg. However, it does not seem possible to
        // retrieve historic block hashes just from the subgraph (it always
        // returns `null`).
        Ok(self
            .0
            .query::<block_number_query::Data>(block_number_query::QUERY, None)
            .await?
            .meta
            .block
            .number
            .saturating_sub(MAX_REORG_BLOCK_COUNT))
    }
}

/// Result of the registered stable pool query.
#[derive(Debug, Default, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

/// Pool data from the Uniswap V3 subgraph.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub id: H160,
    pub token0: Token,
    pub token1: Token,
    #[serde(with = "u256_decimal")]
    pub fee_tier: U256,
    #[serde(with = "u256_decimal")]
    pub liquidity: U256,
    #[serde(with = "u256_decimal")]
    pub sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub tick: BigInt,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "totalValueLockedETH")]
    pub total_value_locked_eth: f64,
    pub ticks: Option<Vec<TickData>>,
}

impl ContainsId for PoolData {
    fn get_id(&self) -> String {
        self.id.to_string()
    }
}

/// Tick data from the Uniswap V3 subgraph.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TickData {
    pub id: String,
    #[serde_as(as = "DisplayFromStr")]
    pub tick_idx: BigInt,
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity_net: BigInt,
    pub pool_address: H160,
}

impl ContainsId for TickData {
    fn get_id(&self) -> String {
        self.id.clone()
    }
}

#[serde_as]
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: H160,
    #[serde_as(as = "DisplayFromStr")]
    pub decimals: u8,
}

mod block_number_query {
    use serde::Deserialize;

    pub const QUERY: &str = r#"{
        _meta {
            block { number }
        }
    }"#;

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Data {
        #[serde(rename = "_meta")]
        pub meta: Meta,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Meta {
        pub block: Block,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Block {
        pub number: u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subgraph::{Data, QUERY_PAGE_SIZE};
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn decode_pools_data() {
        assert_eq!(
            serde_json::from_value::<Data<PoolData>>(json!({
                "pools": [
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28",
                      "token0": {
                        "decimals": "18",
                        "id": "0xbef81556ef066ec840a540595c8d12f516b6378f",
                        "symbol": "BCZ"
                      },
                      "token1": {
                        "decimals": "18",
                        "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "symbol": "WETH"
                      },
                      "feeTier": "10000",
                      "liquidity": "303015134493562686441",
                      "tick": "-92110",
                      "totalValueLockedETH": "1.0",
                      "sqrtPrice": "792216481398733702759960397"
                    },
                    {
                      "id": "0x0002e63328169d7feea121f1e32e4f620abf0352",
                      "token0": {
                        "decimals": "18",
                        "id": "0x0d438f3b5175bebc262bf23753c1e53d03432bde",
                        "symbol": "wNXM"
                      },
                      "token1": {
                        "decimals": "9",
                        "id": "0x903bef1736cddf2a537176cf3c64579c3867a881",
                        "symbol": "ICHI"
                      },
                      "feeTier": "3000",
                      "liquidity": "3125586395511534995",
                      "tick": "-189822",
                      "totalValueLockedETH": "1.0",
                      "sqrtPrice": "5986323062404391218190509"
                    }
                ],
            }))
            .unwrap(),
            Data {
                inner: vec![
                    PoolData {
                        id: H160::from_str("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28").unwrap(),
                        token0: Token {
                            id: H160::from_str("0xbef81556ef066ec840a540595c8d12f516b6378f")
                                .unwrap(),
                            decimals: 18,
                        },
                        token1: Token {
                            id: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                                .unwrap(),
                            decimals: 18,
                        },
                        fee_tier: U256::from_dec_str("10000").unwrap(),
                        liquidity: U256::from_dec_str("303015134493562686441").unwrap(),
                        sqrt_price: U256::from_dec_str("792216481398733702759960397").unwrap(),
                        tick: BigInt::from(-92110),
                        ticks: None,
                        total_value_locked_eth: 1.0
                    },
                    PoolData {
                        id: H160::from_str("0x0002e63328169d7feea121f1e32e4f620abf0352").unwrap(),
                        token0: Token {
                            id: H160::from_str("0x0d438f3b5175bebc262bf23753c1e53d03432bde")
                                .unwrap(),
                            decimals: 18,
                        },
                        token1: Token {
                            id: H160::from_str("0x903bef1736cddf2a537176cf3c64579c3867a881")
                                .unwrap(),
                            decimals: 9,
                        },
                        fee_tier: U256::from_dec_str("3000").unwrap(),
                        liquidity: U256::from_dec_str("3125586395511534995").unwrap(),
                        sqrt_price: U256::from_dec_str("5986323062404391218190509").unwrap(),
                        tick: BigInt::from(-189822),
                        ticks: None,
                        total_value_locked_eth: 1.0
                    },
                ],
            }
        );
    }

    #[test]
    fn decode_ticks_data() {
        assert_eq!(
            serde_json::from_value::<Data<TickData>>(json!({
                "ticks": [
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#0",
                      "tickIdx": "0",
                      "liquidityNet": "-303015134493562686441",
                      "poolAddress": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"
                    },
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#-92200",
                      "tickIdx": "-92200",
                      "liquidityNet": "303015134493562686441",
                      "poolAddress": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"
                    }
                ],
            }))
            .unwrap(),
            Data {
                inner: vec![
                    TickData {
                        id: "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#0".to_string(),
                        tick_idx: BigInt::from(0),
                        liquidity_net: BigInt::from(-303015134493562686441i128),
                        pool_address: H160::from_str("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28")
                            .unwrap(),
                    },
                    TickData {
                        id: "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#-92200".to_string(),
                        tick_idx: BigInt::from(-92200),
                        liquidity_net: BigInt::from(303015134493562686441i128),
                        pool_address: H160::from_str("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28")
                            .unwrap(),
                    },
                ],
            }
        );
    }

    #[test]
    fn decode_block_number_data() {
        use block_number_query::*;

        assert_eq!(
            serde_json::from_value::<Data>(json!({
                "_meta": {
                    "block": {
                        "number": 42,
                    },
                },
            }))
            .unwrap(),
            Data {
                meta: Meta {
                    block: Block { number: 42 }
                }
            }
        );
    }

    #[tokio::test]
    #[ignore]
    async fn get_registered_pools_test() {
        let client = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let result = client.get_registered_pools().await.unwrap();
        println!(
            "Retrieved {} total pools at block {}",
            result.pools.len(),
            result.fetched_block_number,
        );
    }

    #[tokio::test]
    #[ignore]
    async fn get_pools_by_pool_ids_test() {
        let client = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let registered_pools = client.get_registered_pools().await.unwrap();
        let pool_ids = registered_pools
            .pools
            .into_iter()
            .map(|pool| pool.id)
            .take(QUERY_PAGE_SIZE + 10)
            .collect::<Vec<_>>();

        let block_number = registered_pools.fetched_block_number;
        let result = client
            .get_pools_by_pool_ids(&pool_ids, block_number)
            .await
            .unwrap();
        assert_eq!(result.len(), QUERY_PAGE_SIZE + 10);
        assert_eq!(&result.last().unwrap().id, pool_ids.last().unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn get_ticks_by_pools_ids_test() {
        let client = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let block_number = client.get_safe_block().await.unwrap();
        let pool_ids = vec![
            H160::from_str("0x9db9e0e53058c89e5b94e29621a205198648425b").unwrap(),
            H160::from_str("0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8").unwrap(),
        ];
        let result = client
            .get_ticks_by_pools_ids(&pool_ids, block_number)
            .await
            .unwrap();
        dbg!(result);
    }

    #[tokio::test]
    #[ignore]
    async fn get_pools_with_ticks_by_ids_test() {
        let client = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let block_number = client.get_safe_block().await.unwrap();
        let pool_ids = vec![
            H160::from_str("0x9db9e0e53058c89e5b94e29621a205198648425b").unwrap(),
            H160::from_str("0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8").unwrap(),
        ];
        let result = client
            .get_pools_with_ticks_by_ids(&pool_ids, block_number)
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].ticks.is_some());
        assert!(result[1].ticks.is_some());
        dbg!(result);
    }
}
