//! Module for providing Balancer V2 pool liquidity to the solvers.

use {
    crate::{
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances},
            BalancerSwapGivenOutInteraction,
        },
        liquidity::{
            AmmOrderExecution, Liquidity, SettlementHandling, StablePoolOrder, WeightedProductOrder,
        },
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    anyhow::Result,
    contracts::{BalancerV2Vault, GPv2Settlement},
    ethcontract::H256,
    model::TokenPair,
    shared::{
        ethrpc::Web3, http_solver::model::TokenAmount, recent_block_cache::Block,
        sources::balancer_v2::pool_fetching::BalancerPoolFetching,
    },
    std::{collections::HashSet, sync::Arc},
};

/// A liquidity provider for Balancer V2 weighted pools.
pub struct BalancerV2Liquidity {
    settlement: GPv2Settlement,
    vault: BalancerV2Vault,
    pool_fetcher: Arc<dyn BalancerPoolFetching>,
    allowance_manager: Box<dyn AllowanceManaging>,
}

impl BalancerV2Liquidity {
    pub fn new(
        web3: Web3,
        pool_fetcher: Arc<dyn BalancerPoolFetching>,
        settlement: GPv2Settlement,
        vault: BalancerV2Vault,
    ) -> Self {
        let allowance_manager = AllowanceManager::new(web3, settlement.address());
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
            .get_allowances(tokens, self.vault.address())
            .await?;

        let inner = Arc::new(Inner {
            allowances,
            settlement: self.settlement.clone(),
            vault: self.vault.clone(),
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
    pool_id: H256,
    inner: Arc<Inner>,
}

struct Inner {
    settlement: GPv2Settlement,
    vault: BalancerV2Vault,
    allowances: Allowances,
}

impl SettlementHandler {
    pub fn new(
        pool_id: H256,
        settlement: GPv2Settlement,
        vault: BalancerV2Vault,
        allowances: Allowances,
    ) -> Self {
        SettlementHandler {
            pool_id,
            inner: Arc::new(Inner {
                settlement,
                vault,
                allowances,
            }),
        }
    }

    pub fn vault(&self) -> &BalancerV2Vault {
        &self.inner.vault
    }

    pub fn pool_id(&self) -> H256 {
        self.pool_id
    }

    pub fn swap(
        &self,
        input_max: TokenAmount,
        output: TokenAmount,
    ) -> BalancerSwapGivenOutInteraction {
        BalancerSwapGivenOutInteraction {
            settlement: self.inner.settlement.clone(),
            vault: self.inner.vault.clone(),
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
        contracts::dummy_contract,
        maplit::{btreemap, hashmap, hashset},
        mockall::predicate::*,
        model::TokenPair,
        primitive_types::H160,
        shared::{
            baseline_solver::BaseTokens,
            http_solver::model::{InternalizationStrategy, TokenAmount},
            interaction::Interaction,
            sources::balancer_v2::{
                pool_fetching::{
                    AmplificationParameter, CommonPoolState, FetchedBalancerPools,
                    MockBalancerPoolFetching, StablePool, TokenState, WeightedPool,
                    WeightedPoolVersion, WeightedTokenState,
                },
                swap::fixed_point::Bfp,
            },
        },
    };

    fn dummy_contracts() -> (GPv2Settlement, BalancerV2Vault) {
        (
            dummy_contract!(GPv2Settlement, H160([0xc0; 20])),
            dummy_contract!(BalancerV2Vault, H160([0xc1; 20])),
        )
    }

    fn token_pair(seed0: u8, seed1: u8) -> TokenPair {
        TokenPair::new(H160([seed0; 20]), H160([seed1; 20])).unwrap()
    }

    #[tokio::test]
    async fn fetches_liquidity() {
        let mut pool_fetcher = MockBalancerPoolFetching::new();
        let mut allowance_manager = MockAllowanceManaging::new();

        let weighted_pools = vec![
            WeightedPool {
                common: CommonPoolState {
                    id: H256([0x90; 32]),
                    address: H160([0x90; 20]),
                    swap_fee: "0.002".parse().unwrap(),
                    paused: true,
                },
                reserves: btreemap! {
                    H160([0x70; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 100.into(),
                            scaling_factor: Bfp::exp10(16),
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0x71; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000.into(),
                            scaling_factor: Bfp::exp10(12),
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_factor: Bfp::exp10(0),
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                version: WeightedPoolVersion::V0,
            },
            WeightedPool {
                common: CommonPoolState {
                    id: H256([0x91; 32]),
                    address: H160([0x91; 20]),
                    swap_fee: "0.001".parse().unwrap(),
                    paused: true,
                },
                reserves: btreemap! {
                    H160([0x73; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_factor: Bfp::exp10(0),
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
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
                id: H256([0x92; 32]),
                address: H160([0x92; 20]),
                swap_fee: "0.002".parse().unwrap(),
                paused: true,
            },
            amplification_parameter: AmplificationParameter::try_new(1.into(), 1.into()).unwrap(),
            reserves: btreemap! {
                H160([0x73; 20]) => TokenState {
                        balance: 1_000_000_000_000_000_000u128.into(),
                        scaling_factor: Bfp::exp10(0),
                    },
                H160([0xb0; 20]) => TokenState {
                        balance: 1_000_000_000_000_000_000u128.into(),
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
                    H160([0x70; 20]),
                    H160([0x71; 20]),
                    H160([0x73; 20]),
                    H160([0xb0; 20]),
                ]),
                always(),
            )
            .returning(|_, _| Ok(Allowances::empty(H160([0xc1; 20]))));

        let base_tokens = BaseTokens::new(H160([0xb0; 20]), &[]);
        let traded_pairs = [
            TokenPair::new(H160([0x70; 20]), H160([0x71; 20])).unwrap(),
            TokenPair::new(H160([0x70; 20]), H160([0x72; 20])).unwrap(),
            TokenPair::new(H160([0xb0; 20]), H160([0x73; 20])).unwrap(),
        ];
        let pairs = base_tokens.relevant_pairs(traded_pairs.into_iter());

        let (settlement, vault) = dummy_contracts();
        let liquidity_provider = BalancerV2Liquidity {
            settlement,
            vault,
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
            settlement: settlement.clone(),
            vault: vault.clone(),
            allowances: Allowances::new(
                vault.address(),
                hashmap! {
                    H160([0x70; 20]) => 0.into(),
                    H160([0x71; 20]) => 100.into(),
                },
            ),
        });
        let handler = SettlementHandler {
            pool_id: H256([0x90; 32]),
            inner,
        };

        let mut encoder = SettlementEncoder::new(Default::default());
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input_max: TokenAmount::new(H160([0x70; 20]), 10),
                output: TokenAmount::new(H160([0x71; 20]), 11),
                internalizable: false,
            },
            &mut encoder,
        )
        .unwrap();
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input_max: TokenAmount::new(H160([0x71; 20]), 12),
                output: TokenAmount::new(H160([0x72; 20]), 13),
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
                    token: H160([0x70; 20]),
                    spender: vault.address(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement: settlement.clone(),
                    vault: vault.clone(),
                    pool_id: H256([0x90; 32]),
                    asset_in_max: TokenAmount::new(H160([0x70; 20]), 10),
                    asset_out: TokenAmount::new(H160([0x71; 20]), 11),
                    user_data: Default::default(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement,
                    vault,
                    pool_id: H256([0x90; 32]),
                    asset_in_max: TokenAmount::new(H160([0x71; 20]), 12),
                    asset_out: TokenAmount::new(H160([0x72; 20]), 13),
                    user_data: Default::default(),
                }
                .encode(),
            ],
        );
    }
}
