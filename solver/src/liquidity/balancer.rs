//! Module for providing Balancer V2 pool liquidity to the solvers.

use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances},
        BalancerSwapGivenOutInteraction,
    },
    liquidity::{
        slippage, AmmOrderExecution, LimitOrder, SettlementHandling, WeightedProductOrder,
    },
    settlement::SettlementEncoder,
};
use anyhow::{Context as _, Result};
use contracts::{BalancerV2Vault, GPv2Settlement};
use ethcontract::{H160, H256};
use shared::{
    baseline_solver::{relevant_token_pairs, DEFAULT_MAX_HOPS},
    recent_block_cache::Block,
    sources::balancer::pool_fetching::WeightedPoolFetching,
    Web3,
};
use std::{collections::HashSet, sync::Arc};

struct Contracts {
    settlement: GPv2Settlement,
    vault: BalancerV2Vault,
}

impl Contracts {
    async fn new(web3: &Web3) -> Result<Self> {
        let settlement = GPv2Settlement::deployed(web3).await?;
        let vault = BalancerV2Vault::deployed(web3).await?;
        Ok(Self { settlement, vault })
    }
}

/// A liquidity provider for Balancer V2 weighted pools.
pub struct BalancerV2Liquidity {
    contracts: Arc<Contracts>,
    pool_fetcher: Arc<dyn WeightedPoolFetching>,
    allowance_manager: Box<dyn AllowanceManaging>,
    base_tokens: HashSet<H160>,
}

impl BalancerV2Liquidity {
    pub async fn new(
        web3: Web3,
        pool_fetcher: Arc<dyn WeightedPoolFetching>,
        base_tokens: HashSet<H160>,
    ) -> Result<Self> {
        let contracts = Contracts::new(&web3)
            .await
            .context("missing Balancer V2 contract deployement")?;
        let allowance_manager = AllowanceManager::new(web3, contracts.settlement.address());

        Ok(Self {
            contracts: Arc::new(contracts),
            pool_fetcher,
            allowance_manager: Box::new(allowance_manager),
            base_tokens,
        })
    }

    /// Returns relevant Balancer V2 weighted pools given a list of off-chain
    /// orders.
    pub async fn get_liquidity(
        &self,
        orders: &[LimitOrder],
        block: Block,
    ) -> Result<Vec<WeightedProductOrder>> {
        let pairs = orders
            .iter()
            .flat_map(|order| {
                relevant_token_pairs(
                    order.sell_token,
                    order.buy_token,
                    &self.base_tokens,
                    DEFAULT_MAX_HOPS,
                )
            })
            .collect();
        let pools = self.pool_fetcher.fetch(pairs, block).await?;

        let tokens = pools
            .iter()
            .flat_map(|pool| pool.reserves.keys())
            .copied()
            .collect();
        let allowances = Arc::new(
            self.allowance_manager
                .get_allowances(tokens, self.contracts.vault.address())
                .await?,
        );

        let liquidity = pools
            .into_iter()
            .map(|pool| WeightedProductOrder {
                reserves: pool.reserves,
                fee: pool.swap_fee_percentage.into(),
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.pool_id,
                    contracts: self.contracts.clone(),
                    allowances: allowances.clone(),
                }),
            })
            .collect();

        Ok(liquidity)
    }
}

struct SettlementHandler {
    pool_id: H256,
    contracts: Arc<Contracts>,
    allowances: Arc<Allowances>,
}

impl SettlementHandling<WeightedProductOrder> for SettlementHandler {
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let (asset_in, amount_in) = execution.input;
        let (asset_out, amount_out) = execution.output;

        encoder.append_to_execution_plan(self.allowances.approve_token(asset_in, amount_in)?);
        encoder.append_to_execution_plan(BalancerSwapGivenOutInteraction {
            settlement: self.contracts.settlement.clone(),
            vault: self.contracts.vault.clone(),
            pool_id: self.pool_id,
            asset_in,
            asset_out,
            amount_out,
            amount_in_max: slippage::amount_plus_max_slippage(amount_in),
            // Balancer pools allow passing additonal user data in order to
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
    use shared::{
        dummy_contract,
        sources::balancer::pool_fetching::{
            MockWeightedPoolFetching, TokenState, WeightedPool, WeightedTokenState,
        },
    };

    fn dummy_contracts() -> Arc<Contracts> {
        Arc::new(Contracts {
            settlement: dummy_contract!(GPv2Settlement, H160([0xc0; 20])),
            vault: dummy_contract!(BalancerV2Vault, H160([0xc1; 20])),
        })
    }

    fn token_pair(seed0: u8, seed1: u8) -> TokenPair {
        TokenPair::new(H160([seed0; 20]), H160([seed1; 20])).unwrap()
    }

    #[tokio::test]
    async fn fetches_liquidity() {
        let mut pool_fetcher = MockWeightedPoolFetching::new();
        let mut allowance_manager = MockAllowanceManaging::new();

        let pools = vec![
            WeightedPool {
                pool_id: H256([0x90; 32]),
                pool_address: H160([0x90; 20]),
                swap_fee_percentage: "0.002".parse().unwrap(),
                reserves: hashmap! {
                    H160([0x70; 20]) => WeightedTokenState {
                        token_state: TokenState {
                            balance: 100.into(),
                            scaling_exponent: 16,
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0x71; 20]) => WeightedTokenState {
                        token_state: TokenState {
                            balance: 1_000_000.into(),
                            scaling_exponent: 12,
                        },
                        weight: "0.25".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        token_state: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                paused: true,
            },
            WeightedPool {
                pool_id: H256([0x91; 32]),
                pool_address: H160([0x91; 20]),
                swap_fee_percentage: "0.001".parse().unwrap(),
                reserves: hashmap! {
                    H160([0x73; 20]) => WeightedTokenState {
                        token_state: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                    H160([0xb0; 20]) => WeightedTokenState {
                        token_state: TokenState {
                            balance: 1_000_000_000_000_000_000u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                paused: true,
            },
        ];

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
                let pools = pools.clone();
                move |_, _| Ok(pools.clone())
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

        let liquidity_provider = BalancerV2Liquidity {
            contracts: dummy_contracts(),
            pool_fetcher: Arc::new(pool_fetcher),
            allowance_manager: Box::new(allowance_manager),
            base_tokens: hashset![H160([0xb0; 20])],
        };
        let liquidity = liquidity_provider
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

        assert_eq!(liquidity.len(), 2);
        assert_eq!(
            (&liquidity[0].reserves, &liquidity[0].fee),
            (&pools[0].reserves, &BigRational::new(2.into(), 1000.into())),
        );
        assert_eq!(
            (&liquidity[1].reserves, &liquidity[1].fee),
            (&pools[1].reserves, &BigRational::new(1.into(), 1000.into())),
        );
    }

    #[test]
    fn encodes_swaps_in_settlement() {
        let contracts = dummy_contracts();
        let handler = SettlementHandler {
            pool_id: H256([0x90; 32]),
            contracts: contracts.clone(),
            allowances: Arc::new(Allowances::new(
                contracts.vault.address(),
                hashmap! {
                    H160([0x70; 20]) => 0.into(),
                    H160([0x71; 20]) => 100.into(),
                },
            )),
        };

        let mut encoder = SettlementEncoder::new(Default::default());
        handler
            .encode(
                AmmOrderExecution {
                    input: (H160([0x70; 20]), 10.into()),
                    output: (H160([0x71; 20]), 11.into()),
                },
                &mut encoder,
            )
            .unwrap();
        handler
            .encode(
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
                    spender: contracts.vault.address(),
                }
                .encode(),
                BalancerSwapGivenOutInteraction {
                    settlement: contracts.settlement.clone(),
                    vault: contracts.vault.clone(),
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
                    settlement: contracts.settlement.clone(),
                    vault: contracts.vault.clone(),
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
