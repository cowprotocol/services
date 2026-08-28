//! Static Balancer V2 pool data — the registry seed consumed by the aggregate
//! pool fetcher. Populated by the pool-indexer client.

use {
    super::swap::fixed_point::Bfp,
    alloy::primitives::{Address, B256},
    serde::Deserialize,
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

/// A set of registered pools, up to date as of `fetched_block_number`.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct RegisteredPools {
    /// The block the pools were fetched for and can be considered current at.
    pub fetched_block_number: u64,
    /// The registered pools.
    pub pools: Vec<PoolData>,
}

impl RegisteredPools {
    /// Creates an empty collection for the specified block number.
    pub fn empty(fetched_block_number: u64) -> Self {
        Self {
            fetched_block_number,
            ..Default::default()
        }
    }

    /// Groups registered pools by factory address.
    pub fn group_by_factory(self) -> HashMap<Address, RegisteredPools> {
        let fetched_block_number = self.fetched_block_number;
        self.pools
            .into_iter()
            .fold(HashMap::new(), |mut grouped, pool| {
                grouped
                    .entry(pool.factory)
                    .or_insert(RegisteredPools {
                        fetched_block_number,
                        ..Default::default()
                    })
                    .pools
                    .push(pool);
                grouped
            })
    }
}

/// Static data for a Balancer V2 pool.
#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub pool_type: PoolType,
    pub id: B256,
    pub address: Address,
    pub factory: Address,
    pub swap_enabled: bool,
    pub tokens: Vec<Token>,
}

/// Supported pool kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub enum PoolType {
    Stable,
    Weighted,
    LiquidityBootstrapping,
    ComposableStable,
}

/// Token data for pools. `weight` is present only for weighted pools.
#[serde_as]
#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Token {
    pub address: Address,
    pub decimals: u8,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub weight: Option<Bfp>,
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::U256, maplit::hashmap, serde_json::json};

    #[test]
    fn decode_pools_data() {
        assert_eq!(
            serde_json::from_value::<Vec<PoolData>>(json!([
                {
                    "poolType": "Weighted",
                    "address": "0x2222222222222222222222222222222222222222",
                    "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                    "factory": "0x5555555555555555555555555555555555555555",
                    "swapEnabled": true,
                    "tokens": [
                        {
                            "address": "0x3333333333333333333333333333333333333333",
                            "decimals": 3,
                            "weight": "0.5"
                        },
                        {
                            "address": "0x4444444444444444444444444444444444444444",
                            "decimals": 4,
                            "weight": "0.5"
                        },
                    ],
                },
                {
                    "poolType": "Stable",
                    "address": "0x2222222222222222222222222222222222222222",
                    "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                    "factory": "0x5555555555555555555555555555555555555555",
                    "swapEnabled": true,
                    "tokens": [
                        {
                            "address": "0x3333333333333333333333333333333333333333",
                            "decimals": 3,
                        },
                        {
                            "address": "0x4444444444444444444444444444444444444444",
                            "decimals": 4,
                        },
                    ],
                },
                {
                    "poolType": "LiquidityBootstrapping",
                    "address": "0x2222222222222222222222222222222222222222",
                    "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                    "factory": "0x5555555555555555555555555555555555555555",
                    "swapEnabled": true,
                    "tokens": [
                        {
                            "address": "0x3333333333333333333333333333333333333333",
                            "decimals": 3,
                            "weight": "0.5"
                        },
                        {
                            "address": "0x4444444444444444444444444444444444444444",
                            "decimals": 4,
                            "weight": "0.5"
                        },
                    ],
                },
                {
                    "poolType": "ComposableStable",
                    "address": "0x2222222222222222222222222222222222222222",
                    "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                    "factory": "0x5555555555555555555555555555555555555555",
                    "swapEnabled": true,
                    "tokens": [
                        {
                            "address": "0x3333333333333333333333333333333333333333",
                            "decimals": 3,
                        },
                        {
                            "address": "0x4444444444444444444444444444444444444444",
                            "decimals": 4,
                        },
                    ],
                },
            ]))
            .unwrap(),
            vec![
                PoolData {
                    pool_type: PoolType::Weighted,
                    id: B256::repeat_byte(0x11),
                    address: Address::repeat_byte(0x22),
                    factory: Address::repeat_byte(0x55),
                    swap_enabled: true,
                    tokens: vec![
                        Token {
                            address: Address::repeat_byte(0x33),
                            decimals: 3,
                            weight: Some(Bfp::from_wei(U256::from(500_000_000_000_000_000_u128))),
                        },
                        Token {
                            address: Address::repeat_byte(0x44),
                            decimals: 4,
                            weight: Some(Bfp::from_wei(U256::from(500_000_000_000_000_000_u128))),
                        },
                    ],
                },
                PoolData {
                    pool_type: PoolType::Stable,
                    id: B256::repeat_byte(0x11),
                    address: Address::repeat_byte(0x22),
                    factory: Address::repeat_byte(0x55),
                    swap_enabled: true,
                    tokens: vec![
                        Token {
                            address: Address::repeat_byte(0x33),
                            decimals: 3,
                            weight: None,
                        },
                        Token {
                            address: Address::repeat_byte(0x44),
                            decimals: 4,
                            weight: None,
                        },
                    ],
                },
                PoolData {
                    pool_type: PoolType::LiquidityBootstrapping,
                    id: B256::repeat_byte(0x11),
                    address: Address::repeat_byte(0x22),
                    factory: Address::repeat_byte(0x55),
                    swap_enabled: true,
                    tokens: vec![
                        Token {
                            address: Address::repeat_byte(0x33),
                            decimals: 3,
                            weight: Some(Bfp::from_wei(U256::from(500_000_000_000_000_000_u128))),
                        },
                        Token {
                            address: Address::repeat_byte(0x44),
                            decimals: 4,
                            weight: Some(Bfp::from_wei(U256::from(500_000_000_000_000_000_u128))),
                        },
                    ],
                },
                PoolData {
                    pool_type: PoolType::ComposableStable,
                    id: B256::repeat_byte(0x11),
                    address: Address::repeat_byte(0x22),
                    factory: Address::repeat_byte(0x55),
                    swap_enabled: true,
                    tokens: vec![
                        Token {
                            address: Address::repeat_byte(0x33),
                            decimals: 3,
                            weight: None,
                        },
                        Token {
                            address: Address::repeat_byte(0x44),
                            decimals: 4,
                            weight: None,
                        },
                    ],
                },
            ]
        );
    }

    #[test]
    fn groups_pools_by_factory() {
        let pool = |factory: Address, id: u8| PoolData {
            id: B256::repeat_byte(id),
            factory,
            pool_type: PoolType::Weighted,
            address: Default::default(),
            swap_enabled: true,
            tokens: Default::default(),
        };

        let registered_pools = RegisteredPools {
            pools: vec![
                pool(Address::repeat_byte(1), 1),
                pool(Address::repeat_byte(1), 2),
                pool(Address::repeat_byte(2), 3),
            ],
            fetched_block_number: 42,
        };

        assert_eq!(
            registered_pools.group_by_factory(),
            hashmap! {
                Address::repeat_byte(1) => RegisteredPools {
                    pools: vec![
                        pool(Address::repeat_byte(1), 1),
                        pool(Address::repeat_byte(1), 2),
                    ],
                    fetched_block_number: 42,
                },
                Address::repeat_byte(2) => RegisteredPools {
                    pools: vec![
                        pool(Address::repeat_byte(2), 3),
                    ],
                    fetched_block_number: 42,
                },
            }
        )
    }
}
