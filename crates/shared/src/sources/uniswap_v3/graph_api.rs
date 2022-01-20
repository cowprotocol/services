use crate::{subgraph::SubgraphClient, event_handling::MAX_REORG_BLOCK_COUNT};
use anyhow::{Result, bail};
use ethcontract::{H160, U256};
use reqwest::Client;
use serde::{Deserialize};
use serde_json::json;
use serde_with::{serde_as, DisplayFromStr};

/// The page size when querying pools.
#[cfg(not(test))]
const QUERY_PAGE_SIZE: usize = 1000;
#[cfg(test)]
const QUERY_PAGE_SIZE: usize = 10;

pub struct UniswapV3SubgraphClient(SubgraphClient);

impl UniswapV3SubgraphClient {
    /// Creates a new UniswapV3 subgraph client for the specified chain ID.
    pub fn for_chain(chain_id: u64, client: Client) -> Result<Self> {
        let subgraph_name = match chain_id {
            1 => "uniswap-v3",
            4 => "uniswap-rinkeby-v3", // FIXME: made up name
            _ => bail!("unsupported chain {}", chain_id),
        };
        Ok(Self(SubgraphClient::new(
            "uniswap",
            subgraph_name,
            client,
        )?))
    }

    pub async fn get_registered_pools(&self, min_tvl_eth: f64, block_number: u64) -> Result<RegisteredPools> {
        use self::pools_query::*;

        let mut pools = Vec::new();
        let mut last_id = H160::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/graphql-api#pagination>
        loop {
            let page = self
                .0
                .query::<Data>(
                    QUERY,
                    Some(json_map! {
                        "block" => block_number,
                        "pageSize" => QUERY_PAGE_SIZE,
                        "lastId" => json!(last_id),
                        "minTotalValueLockedETH" => min_tvl_eth,
                    }),
                )
                .await?
                .pools;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_pool) = page.last() {
                last_id = last_pool.id;
            }

            pools.extend(page);

            if no_more_pages {
                break;
            }
        }

        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools,
        })
    }

    pub async fn get_pool_ticks(&self, pool_id: H160, block_number: u64) -> Result<Ticks> {
        use self::ticks_query::*;

        let mut ticks = Vec::new();
        let mut last_id = String::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/graphql-api#pagination>
        loop {
            let page = self
                .0
                .query::<Data>(
                    QUERY,
                    Some(json_map! {
                        "block" => block_number,
                        "pageSize" => QUERY_PAGE_SIZE,
                        "lastId" => json!(last_id),
                        "pool" => json!(pool_id),
                    }),
                )
                .await?
                .ticks;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_tick) = page.last() {
                last_id = last_tick.id.clone();
            }

            ticks.extend(page);

            if no_more_pages {
                break;
            }
        }

        Ok(Ticks {
            fetched_block_number: block_number,
            ticks,
        })
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    async fn get_safe_block(&self) -> Result<u64> {
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
    /// The block number that the data was fetched, and for which the registered
    /// weighted pools can be considered up to date.
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

/// Token data for pools.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
pub struct Token {
    #[serde(rename = "id")]
    pub address: H160,
    #[serde_as(as = "DisplayFromStr")]
    pub decimals: u32,
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub id: H160,
    pub token0: Token,
    pub token1: Token,
    #[serde_as(as = "DisplayFromStr")]
    pub fee_tier: u32,
    pub liquidity: U256,    // u128 in the paper
    pub sqrt_price: U256,   // u160 in the paper
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub tick: Option<i32>,          // i24 in the paper        
}

mod pools_query {
    use serde::Deserialize;

    use super::PoolData;

    pub const QUERY: &str = r#"
    query Pools($block: Int, $pageSize: Int, $lastId: ID, $minTotalValueLockedETH: BigDecimal) {
        pools(
            block: { number: $block }
            first: $pageSize
            where: {
                id_gt: $lastId
                totalValueLockedETH_gt: $minTotalValueLockedETH
            }
        ) {
            id
            token0 {
                id
                decimals
            }
            token1 {
                id
                decimals
            }
            feeTier
            liquidity
            sqrtPrice
            tick
        }
    }
    "#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        pub pools: Vec<PoolData>,
    }
}


/// Result of the ticks query.
#[derive(Debug, Default, PartialEq)]
pub struct Ticks {
    /// The block number that the data was fetched, and for which the 
    /// tick info are considered up to date.
    pub fetched_block_number: u64,
    /// The registered Pools
    pub ticks: Vec<TickData>,
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TickData {
    pub id: String,
    #[serde_as(as = "DisplayFromStr")]
    pub tick_idx: i32,  // i24 in the paper
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity_net: i128,
}

mod ticks_query {
    use serde::Deserialize;

    use super::TickData;

    pub const QUERY: &str = r#"
    query Ticks($pool: ID, $block: Int, $pageSize: Int, $lastId: ID) {
        ticks(
            block: { number: $block }
            first: $pageSize
            where:{
                id_gt: $lastId
    	        pool: $pool
    	        liquidityNet_not: 0
            }
        ) {
            id
            tickIdx
            liquidityNet
        }
    }
    "#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        pub ticks: Vec<TickData>,
    }
}

mod block_number_query {
    use serde::Deserialize;

    pub const QUERY: &str = r#"{
        _meta {
            block { number }
        }
    }"#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        #[serde(rename = "_meta")]
        pub meta: Meta,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Meta {
        pub block: Block,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Block {
        pub number: u64,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn for_chain() {
        UniswapV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        UniswapV3SubgraphClient::for_chain(4, Client::new()).unwrap();
    }

    #[tokio::test]
    async fn get_registered_pools() {
        let client = UniswapV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let block_number = 14037316;

        let registered_pools = client.get_registered_pools(10000f64, block_number).await.unwrap();
        assert_eq!(registered_pools.pools.len(), 22);

        let first_pool = &registered_pools.pools[0];
        assert_eq!(first_pool.id, H160::from_str("0x00cef0386ed94d738c8f8a74e8bfd0376926d24c").unwrap());
        assert_eq!(first_pool.token0.address, H160::from_str("0x4fabb145d64652a948d72533023f6e7a623c7c53").unwrap());
        assert_eq!(first_pool.token0.decimals, 18);
        assert_eq!(first_pool.token1.address, H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap());
        assert_eq!(first_pool.token1.decimals, 6);
        assert_eq!(first_pool.fee_tier, 500);
        assert_eq!(first_pool.liquidity, U256::from_dec_str("5717694983874928968409368916").unwrap());
        assert_eq!(first_pool.sqrt_price, U256::from_dec_str("2342989830201273967729410374").unwrap());
        assert_eq!(first_pool.tick.unwrap(), -276328);
    }

    #[tokio::test]
    async fn get_ticks() {
        let client = UniswapV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let block_number = 14037316;

        let pool_id = H160::from_str("0x00cef0386ed94d738c8f8a74e8bfd0376926d24c").unwrap();

        println!("block {}", block_number);
        let ticks = client.get_pool_ticks(pool_id, block_number).await.unwrap();
        println!("{:#?}", ticks);
    }

}
