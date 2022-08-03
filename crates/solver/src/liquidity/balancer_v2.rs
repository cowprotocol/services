//! Module for providing Balancer V2 pool liquidity to the solvers.

use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances},
        BalancerSwapGivenOutInteraction,
    },
    liquidity::{
        slippage, AmmOrderExecution, LimitOrder, SettlementHandling, StablePoolOrder,
        WeightedProductOrder,
    },
    settlement::SettlementEncoder,
};
use anyhow::Result;
use contracts::{BalancerV2Vault, GPv2Settlement};
use ethcontract::H256;
use model::TokenPair;
use shared::{
    baseline_solver::BaseTokens, recent_block_cache::Block,
    sources::balancer_v2::pool_fetching::BalancerPoolFetching, Web3,
};
use std::sync::Arc;

/// A liquidity provider for Balancer V2 weighted pools.
pub struct BalancerV2Liquidity {
    settlement: GPv2Settlement,
    vault: BalancerV2Vault,
    pool_fetcher: Arc<dyn BalancerPoolFetching>,
    allowance_manager: Box<dyn AllowanceManaging>,
    base_tokens: Arc<BaseTokens>,
}

impl BalancerV2Liquidity {
    pub fn new(
        web3: Web3,
        pool_fetcher: Arc<dyn BalancerPoolFetching>,
        base_tokens: Arc<BaseTokens>,
        settlement: GPv2Settlement,
        vault: BalancerV2Vault,
    ) -> Self {
        let allowance_manager = AllowanceManager::new(web3, settlement.address());
        Self {
            settlement,
            vault,
            pool_fetcher,
            allowance_manager: Box::new(allowance_manager),
            base_tokens,
        }
    }

    /// Returns relevant Balancer V2 weighted pools given a list of off-chain
    /// orders.
    pub async fn get_liquidity(
        &self,
        orders: &[LimitOrder],
        block: Block,
    ) -> Result<(Vec<StablePoolOrder>, Vec<WeightedProductOrder>)> {
        let pairs = self.base_tokens.relevant_pairs(
            &mut orders
                .iter()
                .flat_map(|order| TokenPair::new(order.buy_token, order.sell_token)),
        );
        let pools = self.pool_fetcher.fetch(pairs, block).await?;

        let tokens = pools.relevant_tokens();
        let allowances = Arc::new(
            self.allowance_manager
                .get_allowances(tokens, self.vault.address())
                .await?,
        );

        let weighted_product_orders = pools
            .weighted_pools
            .into_iter()
            .map(|pool| WeightedProductOrder {
                reserves: pool.reserves,
                fee: pool.common.swap_fee,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    settlement: self.settlement.clone(),
                    vault: self.vault.clone(),
                    allowances: allowances.clone(),
                }),
            })
            .collect();
        let stable_pool_orders = pools
            .stable_pools
            .into_iter()
            .map(|pool| StablePoolOrder {
                reserves: pool.reserves,
                fee: pool.common.swap_fee.into(),
                amplification_parameter: pool.amplification_parameter,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    settlement: self.settlement.clone(),
                    vault: self.vault.clone(),
                    allowances: allowances.clone(),
                }),
            })
            .collect();

        Ok((stable_pool_orders, weighted_product_orders))
    }
}

pub struct SettlementHandler {
    pool_id: H256,
    settlement: GPv2Settlement,
    vault: BalancerV2Vault,
    allowances: Arc<Allowances>,
}

#[cfg(test)]
impl SettlementHandler {
    pub fn new(
        pool_id: H256,
        settlement: GPv2Settlement,
        vault: BalancerV2Vault,
        allowances: Arc<Allowances>,
    ) -> Self {
        SettlementHandler {
            pool_id,
            settlement,
            vault,
            allowances,
        }
    }
}

impl SettlementHandling<WeightedProductOrder> for SettlementHandler {
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        self.inner_encode(execution, encoder)
    }
}

impl SettlementHandling<StablePoolOrder> for SettlementHandler {
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
        let (asset_in, amount_in) = execution.input;
        let (asset_out, amount_out) = execution.output;

        encoder.append_to_execution_plan(self.allowances.approve_token(asset_in, amount_in)?);
        encoder.append_to_execution_plan(BalancerSwapGivenOutInteraction {
            settlement: self.settlement.clone(),
            vault: self.vault.clone(),
            pool_id: self.pool_id,
            asset_in,
            asset_out,
            amount_out,
            amount_in_max: slippage::amount_plus_max_slippage(amount_in),
            // Balancer pools allow passing additional user data in order to
            // control pool behaviour for swaps. That being said, weighted pools
            // do not seem to make use of this at the moment so leave it empty.
            user_data: Default::default(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interactions::allowances::{Approval, MockAllowanceManaging},
        settlement::Interaction,
    };
    use maplit::{hashmap, hashset};
    use mockall::predicate::*;
    use model::TokenPair;
    use num::BigRational;
    use primitive_types::H160;
    use shared::sources::balancer_v2::pool_fetching::AmplificationParameter;
    use shared::{
        dummy_contract,
        sources::balancer_v2::pool_fetching::{CommonPoolState, FetchedBalancerPools},
        sources::balancer_v2::pool_fetching::{
            MockBalancerPoolFetching, StablePool, TokenState, WeightedPool, WeightedTokenState,
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
                reserves: hashmap! {
                    H160([0x70; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 100.into(),
                            scaling_exponent: 16,
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0x71; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000.into(),
                            scaling_exponent: 12,
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
            },
            WeightedPool {
                common: CommonPoolState {
                    id: H256([0x91; 32]),
                    address: H160([0x91; 20]),
                    swap_fee: "0.001".parse().unwrap(),
                    paused: true,
                },
                reserves: hashmap! {
                    H160([0x73; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        common: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
            },
        ];

        let stable_pools = vec![StablePool {
            common: CommonPoolState {
                id: H256([0x92; 32]),
                address: H160([0x92; 20]),
                swap_fee: "0.002".parse().unwrap(),
                paused: true,
            },
            amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            reserves: hashmap! {
                H160([0x73; 20]) => TokenState {
                        balance: 1_000_000_000_000_000_000u128.into(),
                        scaling_exponent: 0,
                    },
                H160([0xb0; 20]) => TokenState {
                        balance: 1_000_000_000_000_000_000u128.into(),
                        scaling_exponent: 0,
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

        let base_tokens = Arc::new(BaseTokens::new(H160([0xb0; 20]), &[]));
        let (settlement, vault) = dummy_contracts();
        let liquidity_provider = BalancerV2Liquidity {
            settlement,
            vault,
            pool_fetcher: Arc::new(pool_fetcher),
            allowance_manager: Box::new(allowance_manager),
            base_tokens,
        };
        let (stable_orders, weighted_orders) = liquidity_provider
            .get_liquidity(
                &[
                    LimitOrder {
                        sell_token: H160([0x70; 20]),
                        buy_token: H160([0x71; 20]),
                        ..Default::default()
                    },
                    LimitOrder {
                        sell_token: H160([0x70; 20]),
                        buy_token: H160([0x72; 20]),
                        ..Default::default()
                    },
                    LimitOrder {
                        sell_token: H160([0xb0; 20]),
                        buy_token: H160([0x73; 20]),
                        ..Default::default()
                    },
                ],
                Block::Recent,
            )
            .await
            .unwrap();

        assert_eq!(weighted_orders.len(), 2);
        assert_eq!(stable_orders.len(), 1);

        assert_eq!(
            (&weighted_orders[0].reserves, &weighted_orders[0].fee),
            (&weighted_pools[0].reserves, &"0.002".parse().unwrap()),
        );
        assert_eq!(
            (&weighted_orders[1].reserves, &weighted_orders[1].fee),
            (&weighted_pools[1].reserves, &"0.001".parse().unwrap()),
        );
        assert_eq!(
            (&stable_orders[0].reserves, &stable_orders[0].fee),
            (
                &stable_pools[0].reserves,
                &BigRational::new(2.into(), 1000.into())
            ),
        );
    }

    #[test]
    fn encodes_swaps_in_settlement() {
        let (settlement, vault) = dummy_contracts();
        let handler = SettlementHandler {
            pool_id: H256([0x90; 32]),
            settlement: settlement.clone(),
            vault: vault.clone(),
            allowances: Arc::new(Allowances::new(
                vault.address(),
                hashmap! {
                    H160([0x70; 20]) => 0.into(),
                    H160([0x71; 20]) => 100.into(),
                },
            )),
        };

        let mut encoder = SettlementEncoder::new(Default::default());
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input: (H160([0x70; 20]), 10.into()),
                output: (H160([0x71; 20]), 11.into()),
            },
            &mut encoder,
        )
        .unwrap();
        SettlementHandling::<WeightedProductOrder>::encode(
            &handler,
            AmmOrderExecution {
                input: (H160([0x71; 20]), 12.into()),
                output: (H160([0x72; 20]), 13.into()),
            },
            &mut encoder,
        )
        .unwrap();

        let [_, interactions, _] = encoder.finish().interactions;
        assert_eq!(
            interactions,
            [
                Approval::Approve {
                    token: H160([0x70; 20]),
                    spender: vault.address(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement: settlement.clone(),
                    vault: vault.clone(),
                    pool_id: H256([0x90; 32]),
                    asset_in: H160([0x70; 20]),
                    asset_out: H160([0x71; 20]),
                    amount_out: 11.into(),
                    amount_in_max: slippage::amount_plus_max_slippage(10.into()),
                    user_data: Default::default(),
                }
                .encode(),
                Approval::AllowanceSufficient.encode(),
                BalancerSwapGivenOutInteraction {
                    settlement,
                    vault,
                    pool_id: H256([0x90; 32]),
                    asset_in: H160([0x71; 20]),
                    asset_out: H160([0x72; 20]),
                    amount_out: 13.into(),
                    amount_in_max: slippage::amount_plus_max_slippage(12.into()),
                    user_data: Default::default(),
                }
                .encode(),
            ]
            .concat(),
        );
    }
}
