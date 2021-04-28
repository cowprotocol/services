use anyhow::Result;
use contracts::{GPv2Settlement, IUniswapLikeRouter, ERC20};
use ethcontract::batch::CallBatch;
use primitive_types::{H160, U256};
use shared::{
    baseline_solver::{path_candidates, token_path_to_pair_path},
    pool_fetching::{PoolFetcher, PoolFetching as _},
    Web3,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

const MAX_BATCH_SIZE: usize = 100;
pub const MAX_HOPS: usize = 2;

use super::slippage;
use crate::{interactions::UniswapInteraction, settlement::SettlementEncoder};

use super::{AmmOrder, AmmOrderExecution, LimitOrder, SettlementHandling};
use shared::amm_pair_provider::AmmPairProvider;

pub struct UniswapLikeLiquidity {
    inner: Arc<Inner>,
    pool_fetcher: PoolFetcher,
    web3: Web3,
    base_tokens: HashSet<H160>,
}

struct Inner {
    router: IUniswapLikeRouter,
    gpv2_settlement: GPv2Settlement,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement contract
    allowances: Mutex<HashMap<H160, U256>>,
}

impl UniswapLikeLiquidity {
    pub fn new(
        router: IUniswapLikeRouter,
        pair_provider: Arc<dyn AmmPairProvider>,
        gpv2_settlement: GPv2Settlement,
        base_tokens: HashSet<H160>,
        web3: Web3,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                router,
                gpv2_settlement,
                allowances: Mutex::new(HashMap::new()),
            }),
            web3: web3.clone(),
            pool_fetcher: PoolFetcher {
                pair_provider,
                web3,
            },
            base_tokens,
        }
    }

    /// Given a list of offchain orders returns the list of AMM liquidity to be considered
    pub async fn get_liquidity(
        &self,
        offchain_orders: impl Iterator<Item = &LimitOrder> + Send + Sync,
    ) -> Result<Vec<AmmOrder>> {
        let mut pools = HashSet::new();

        for order in offchain_orders {
            let path_candidates = path_candidates(
                order.sell_token,
                order.buy_token,
                &self.base_tokens,
                MAX_HOPS,
            );
            pools.extend(
                path_candidates
                    .iter()
                    .flat_map(|candidate| token_path_to_pair_path(&candidate).into_iter()),
            );
        }

        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(pools).await {
            tokens.insert(pool.tokens.get().0);
            tokens.insert(pool.tokens.get().1);

            result.push(AmmOrder {
                tokens: pool.tokens,
                reserves: pool.reserves,
                fee: pool.fee,
                settlement_handling: self.inner.clone(),
            })
        }
        self.cache_allowances(tokens.into_iter()).await;
        Ok(result)
    }

    async fn cache_allowances(&self, tokens: impl Iterator<Item = H160>) {
        let mut batch = CallBatch::new(self.web3.transport());
        let results: Vec<_> = tokens
            .map(|token| {
                let allowance = ERC20::at(&self.web3, token)
                    .allowance(
                        self.inner.gpv2_settlement.address(),
                        self.inner.router.address(),
                    )
                    .batch_call(&mut batch);
                (token, allowance)
            })
            .collect();

        let _ = batch.execute_all(MAX_BATCH_SIZE).await;

        // await before acquiring lock so we can use std::sync::mutex (async::mutex wouldn't allow AmmSettlementHandling to be non-blocking)
        let mut token_and_allowance = Vec::with_capacity(results.len());
        for (pair, allowance) in results {
            token_and_allowance.push((pair, allowance.await.unwrap_or_default()));
        }

        self.inner
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .extend(token_and_allowance);
    }
}

impl Inner {
    fn _settle(&self, input: (H160, U256), output: (H160, U256)) -> UniswapInteraction {
        let set_allowance = self
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .get(&input.0)
            .cloned()
            .unwrap_or_default()
            < input.1;

        UniswapInteraction {
            router: self.router.clone(),
            settlement: self.gpv2_settlement.clone(),
            set_allowance,
            amount_in: input.1,
            // Apply fixed slippage tolerance in case balances change between solution finding and mining
            amount_out_min: slippage::amount_with_max_slippage(output.1),
            token_in: input.0,
            token_out: output.0,
        }
    }
}

impl SettlementHandling<AmmOrder> for Inner {
    // Creates the required interaction to convert the given input into output. Applies 0.1% slippage tolerance to the output.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        encoder.append_to_execution_plan(self._settle(execution.input, execution.output));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::dummy_web3;

    impl Inner {
        fn new(allowances: HashMap<H160, U256>) -> Self {
            let web3 = dummy_web3::dummy_web3();
            Self {
                router: IUniswapLikeRouter::at(&web3, H160::zero()),
                gpv2_settlement: GPv2Settlement::at(&web3, H160::zero()),
                allowances: Mutex::new(allowances),
            }
        }
    }

    #[test]
    fn test_should_set_allowance() {
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);
        let allowances = maplit::hashmap! {
            token_a => 100.into(),
            token_b => 200.into(),
        };

        let inner = Inner::new(allowances);

        // Token A below, equal, above
        let interaction = inner._settle((token_a, 50.into()), (token_b, 100.into()));
        assert_eq!(interaction.set_allowance, false);

        let interaction = inner._settle((token_a, 100.into()), (token_b, 100.into()));
        assert_eq!(interaction.set_allowance, false);

        let interaction = inner._settle((token_a, 150.into()), (token_b, 100.into()));
        assert_eq!(interaction.set_allowance, true);

        // Token B below, equal, above
        let interaction = inner._settle((token_b, 150.into()), (token_a, 100.into()));
        assert_eq!(interaction.set_allowance, false);

        let interaction = inner._settle((token_b, 200.into()), (token_a, 100.into()));
        assert_eq!(interaction.set_allowance, false);

        let interaction = inner._settle((token_b, 250.into()), (token_a, 100.into()));
        assert_eq!(interaction.set_allowance, true);

        // Untracked token
        let interaction =
            inner._settle((H160::from_low_u64_be(3), 1.into()), (token_a, 100.into()));
        assert_eq!(interaction.set_allowance, true);
    }
}
