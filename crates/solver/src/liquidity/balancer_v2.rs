//! Module for providing Balancer V2 pool liquidity to the solvers.

use {
    crate::{
        interactions::{
            BalancerSwapGivenOutInteraction,
            allowances::{AllowanceManager, AllowanceManaging, Allowances},
        },
        liquidity::{
            AmmOrderExecution,
            Liquidity,
            SettlementHandling,
            StablePoolOrder,
            WeightedProductOrder,
        },
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    alloy::primitives::{Address, B256},
    anyhow::Result,
    model::TokenPair,
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        sources::balancer_v2::pool_fetching::BalancerPoolFetching,
    },
    std::{collections::HashSet, sync::Arc},
    tracing::instrument,
};

/// A liquidity provider for Balancer V2 weighted pools.
pub struct BalancerV2Liquidity {
    settlement: Address,
    vault: Address,
    pool_fetcher: Arc<dyn BalancerPoolFetching>,
    allowance_manager: Box<dyn AllowanceManaging>,
}

impl BalancerV2Liquidity {
    pub fn new(
        web3: Web3,
        pool_fetcher: Arc<dyn BalancerPoolFetching>,
        settlement: Address,
        vault: Address,
    ) -> Self {
        let allowance_manager = AllowanceManager::new(web3, settlement);
        Self {
            settlement,
            vault,
            pool_fetcher,
            allowance_manager: Box::new(allowance_manager),
        }
    }

    async fn get_orders(
        &self,
        pairs: HashSet<TokenPair>,
        block: Block,
    ) -> Result<(Vec<StablePoolOrder>, Vec<WeightedProductOrder>)> {
        let pools = self.pool_fetcher.fetch(pairs, block).await?;

        let tokens = pools.relevant_tokens();

        let allowances = self
            .allowance_manager
            .get_allowances(tokens, self.vault)
            .await?;

        let inner = Arc::new(Inner {
            allowances,
            settlement: self.settlement,
            vault: self.vault,
        });

        let weighted_product_orders: Vec<_> = pools
            .weighted_pools
            .into_iter()
            .map(|pool| WeightedProductOrder {
                address: pool.common.address,
                reserves: pool.reserves,
                fee: pool.common.swap_fee,
                version: pool.version,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    inner: inner.clone(),
                }),
            })
            .collect();
        let stable_pool_orders: Vec<_> = pools
            .stable_pools
            .into_iter()
            .map(|pool| StablePoolOrder {
                address: pool.common.address,
                reserves: pool.reserves,
                fee: pool.common.swap_fee,
                amplification_parameter: pool.amplification_parameter,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    inner: inner.clone(),
                }),
            })
            .collect();

        Ok((stable_pool_orders, weighted_product_orders))
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for BalancerV2Liquidity {
    /// Returns relevant Balancer V2 weighted pools given a list of off-chain
    /// orders.
    #[instrument(name = "balancer_v2_liquidity", skip_all)]
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        block: Block,
    ) -> Result<Vec<Liquidity>> {
        let (stable, weighted) = self.get_orders(pairs, block).await?;
        let liquidity = stable
            .into_iter()
            .map(Liquidity::BalancerStable)
            .chain(weighted.into_iter().map(Liquidity::BalancerWeighted))
            .collect();
        Ok(liquidity)
    }
}

pub struct SettlementHandler {
    pool_id: B256,
    inner: Arc<Inner>,
}

struct Inner {
    settlement: Address,
    vault: Address,
    allowances: Allowances,
}

impl SettlementHandler {
    pub fn new(pool_id: B256, settlement: Address, vault: Address, allowances: Allowances) -> Self {
        SettlementHandler {
            pool_id,
            inner: Arc::new(Inner {
                settlement,
                vault,
                allowances,
            }),
        }
    }

    pub fn vault(&self) -> &Address {
        &self.inner.vault
    }

    pub fn pool_id(&self) -> B256 {
        self.pool_id
    }

    pub fn swap(
        &self,
        input_max: TokenAmount,
        output: TokenAmount,
    ) -> BalancerSwapGivenOutInteraction {
        BalancerSwapGivenOutInteraction {
            settlement: self.inner.settlement,
            vault: self.inner.vault,
            pool_id: self.pool_id,
            asset_in_max: input_max,
            asset_out: output,
            // Balancer pools allow passing additional user data in order to
            // control pool behaviour for swaps. That being said, weighted pools
            // do not seem to make use of this at the moment so leave it empty.
            user_data: Default::default(),
        }
    }
}

impl SettlementHandling<WeightedProductOrder> for SettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        self.inner_encode(execution, encoder)
    }
}

impl SettlementHandling<StablePoolOrder> for SettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        self.inner_encode(execution, encoder)
    }
}

impl SettlementHandler {
    fn inner_encode(
        &self,
        execution: AmmOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        if let Some(approval) = self
            .inner
            .allowances
            .approve_token(execution.input_max.clone())?
        {
            encoder.append_to_execution_plan_internalizable(
                Arc::new(approval),
                execution.internalizable,
            );
        }
        encoder.append_to_execution_plan_internalizable(
            Arc::new(self.swap(execution.input_max, execution.output)),
            execution.internalizable,
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::interactions::allowances::{Approval, MockAllowanceManaging},
        alloy::primitives::U256,
        contracts::alloy::BalancerV2Vault,
        maplit::{btreemap, hashmap, hashset},
        mockall::predicate::*,
        model::TokenPair,
        shared::{
            baseline_solver::BaseTokens,
            http_solver::model::{InternalizationStrategy, TokenAmount},
            interaction::Interaction,
            sources::balancer_v2::{
                pool_fetching::{
                    AmplificationParameter,
                    CommonPoolState,
                    FetchedBalancerPools,
                    MockBalancerPoolFetching,
                    StablePool,
                    TokenState,
                    WeightedPool,
                    WeightedPoolVersion,
                    WeightedTokenState,
                },
                swap::fixed_point::Bfp,
            },
        },
    };

    fn dummy_contracts() -> (Address, BalancerV2Vault::Instance) {
        (
            Address::from_slice(&[0xc0; 20]),
            BalancerV2Vault::Instance::new([0xc1; 20].into(), ethrpc::mock::web3().alloy),
        )
    }

    fn token_pair(seed0: u8, seed1: u8) -> TokenPair {
        TokenPair::new(
            Address::from_slice(&[seed0; 20]),
            Address::from_slice(&[seed1; 20]),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn fetches_liquidity() {
        let mut pool_fetcher = MockBalancerPoolFetching::new();
        let mut allowance_manager = MockAllowanceManaging::new();

        let weighted_pools = vec![
            WeightedPool {
                common: CommonPoolState {
                    id: B256::repeat_byte(0x90),
                    address: Address::repeat_byte(0x90),
                    swap_fee: "0.002".parse().unwrap(),
                    paused: true,
                },
                reserves: btreemap! {
                    Address::repeat_byte(0x70) => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(100),
                            scaling_factor: Bfp::exp10(16),
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    Address::repeat_byte(0x71) => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(1_000_000),
                            scaling_factor: Bfp::exp10(12),
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    Address::repeat_byte(0xb0) => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(1_000_000_000_000_000_000u128),
                            scaling_factor: Bfp::exp10(0),
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                version: WeightedPoolVersion::V0,
            },
            WeightedPool {
                common: CommonPoolState {
                    id: B256::repeat_byte(0x91),
                    address: Address::repeat_byte(0x91),
                    swap_fee: "0.001".parse().unwrap(),
                    paused: true,
                },
                reserves: btreemap! {
                    Address::repeat_byte(0x73) => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(1_000_000_000_000_000_000u128),
                            scaling_factor: Bfp::exp10(0),
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                    Address::repeat_byte(0xb0) => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(1_000_000_000_000_000_000u128),
                            scaling_factor: Bfp::exp10(0),
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                version: WeightedPoolVersion::V3Plus,
            },
        ];

        let stable_pools = vec![StablePool {
            common: CommonPoolState {
                id: B256::repeat_byte(0x92),
                address: Address::repeat_byte(0x92),
                swap_fee: "0.002".parse().unwrap(),
                paused: true,
            },
            amplification_parameter: AmplificationParameter::try_new(U256::ONE, U256::ONE).unwrap(),
            reserves: btreemap! {
                Address::repeat_byte(0x73) => TokenState {
                        balance: U256::from(1_000_000_000_000_000_000u128),
                        scaling_factor: Bfp::exp10(0),
                    },
                Address::repeat_byte(0xb0) => TokenState {
                        balance: U256::from(1_000_000_000_000_000_000u128),
                        scaling_factor: Bfp::exp10(0),
                    }
            },
        }];

        // Fetches pools for all relevant tokens, in this example, there is no
        // pool for token 0x72..72.
        pool_fetcher
            .expect_fetch()
            .with(
                eq(hashset![
                    token_pair(0x70, 0x71),
                    token_pair(0x70, 0xb0),
                    token_pair(0xb0, 0x71),
                    token_pair(0x70, 0x72),
                    token_pair(0xb0, 0x72),
                    token_pair(0xb0, 0x73),
                ]),
                always(),
            )
            .returning({
                let weighted_pools = weighted_pools.clone();
                let stable_pools = stable_pools.clone();
                move |_, _| {
                    Ok(FetchedBalancerPools {
                        stable_pools: stable_pools.clone(),
                        weighted_pools: weighted_pools.clone(),
                    })
                }
            });

        // Fetches allowances for all tokens in pools.
        allowance_manager
            .expect_get_allowances()
            .with(
                eq(hashset![
                    Address::repeat_byte(0x70),
                    Address::repeat_byte(0x71),
                    Address::repeat_byte(0x73),
                    Address::repeat_byte(0xb0),
                ]),
                always(),
            )
            .returning(|_, _| Ok(Allowances::empty(Address::repeat_byte(0xc1))));

        let base_tokens = BaseTokens::new(Address::repeat_byte(0xb0), &[]);
        let traded_pairs = [
            TokenPair::new(
                Address::from_slice(&[0x70; 20]),
                Address::from_slice(&[0x71; 20]),
            )
            .unwrap(),
            TokenPair::new(
                Address::from_slice(&[0x70; 20]),
                Address::from_slice(&[0x72; 20]),
            )
            .unwrap(),
            TokenPair::new(
                Address::from_slice(&[0xb0; 20]),
                Address::from_slice(&[0x73; 20]),
            )
            .unwrap(),
        ];
        let pairs = base_tokens.relevant_pairs(traded_pairs.into_iter());

        let (settlement, vault) = dummy_contracts();
        let liquidity_provider = BalancerV2Liquidity {
            settlement,
            vault: *vault.address(),
            pool_fetcher: Arc::new(pool_fetcher),
            allowance_manager: Box::new(allowance_manager),
        };
        let (stable_orders, weighted_orders) = liquidity_provider
            .get_orders(pairs, Block::Recent)
            .await
            .unwrap();

        assert_eq!(weighted_orders.len(), 2);
        assert_eq!(stable_orders.len(), 1);

        assert_eq!(
            (
                &weighted_orders[0].reserves,
                &weighted_orders[0].fee,
                weighted_orders[0].version
            ),
            (
                &weighted_pools[0].reserves,
                &"0.002".parse().unwrap(),
                WeightedPoolVersion::V0
            ),
        );
        assert_eq!(
            (
                &weighted_orders[1].reserves,
                &weighted_orders[1].fee,
                weighted_orders[1].version
            ),
            (
                &weighted_pools[1].reserves,
                &"0.001".parse().unwrap(),
                WeightedPoolVersion::V3Plus
            ),
        );
        assert_eq!(
            (&stable_orders[0].reserves, &stable_orders[0].fee),
            (&stable_pools[0].reserves, &"0.002".parse().unwrap()),
        );
    }

    #[test]
    fn encodes_swaps_in_settlement() {
        let (settlement, vault) = dummy_contracts();
        let inner = Arc::new(Inner {
            settlement,
            vault: *vault.address(),
            allowances: Allowances::new(
                *vault.address(),
                hashmap! {
                    Address::repeat_byte(0x70) => U256::from(0),
                    Address::repeat_byte(0x71) =>  U256::from(100),
                },
            ),
        });
        let handler = SettlementHandler {
            pool_id: B256::repeat_byte(0x90),
            inner,
        };

        let mut encoder = SettlementEncoder::new(Default::default());
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input_max: TokenAmount::new(Address::repeat_byte(0x70), U256::from(10)),
                output: TokenAmount::new(Address::repeat_byte(0x71), U256::from(11)),
                internalizable: false,
            },
            &mut encoder,
        )
        .unwrap();
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input_max: TokenAmount::new(Address::repeat_byte(0x71), U256::from(12)),
                output: TokenAmount::new(Address::repeat_byte(0x72), U256::from(13)),
                internalizable: false,
            },
            &mut encoder,
        )
        .unwrap();

        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [
                Approval {
                    token: Address::repeat_byte(0x70),
                    spender: *vault.address(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement,
                    vault: *vault.address(),
                    pool_id: B256::repeat_byte(0x90),
                    asset_in_max: TokenAmount::new(Address::repeat_byte(0x70), U256::from(10)),
                    asset_out: TokenAmount::new(Address::repeat_byte(0x71), U256::from(11)),
                    user_data: Default::default(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement,
                    vault: *vault.address(),
                    pool_id: B256::repeat_byte(0x90),
                    asset_in_max: TokenAmount::new(Address::repeat_byte(0x71), U256::from(12)),
                    asset_out: TokenAmount::new(Address::repeat_byte(0x72), U256::from(13)),
                    user_data: Default::default(),
                }
                .encode(),
            ],
        );
    }
}
