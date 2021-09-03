use super::{slippage, AmmOrderExecution, ConstantProductOrder, LimitOrder, SettlementHandling};
use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval},
        UniswapInteraction,
    },
    settlement::SettlementEncoder,
};
use anyhow::Result;
use contracts::{GPv2Settlement, IUniswapLikeRouter};
use primitive_types::{H160, U256};
use shared::{
    baseline_solver::{path_candidates, token_path_to_pair_path, DEFAULT_MAX_HOPS},
    recent_block_cache::Block,
    sources::uniswap::pool_fetching::PoolFetching,
    Web3,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct UniswapLikeLiquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
    settlement_allowances: Box<dyn AllowanceManaging>,
    base_tokens: HashSet<H160>,
}

struct Inner {
    router: IUniswapLikeRouter,
    gpv2_settlement: GPv2Settlement,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement contract
    allowances: Mutex<Allowances>,
}

impl UniswapLikeLiquidity {
    pub fn new(
        router: IUniswapLikeRouter,
        gpv2_settlement: GPv2Settlement,
        base_tokens: HashSet<H160>,
        web3: Web3,
        pool_fetcher: Arc<dyn PoolFetching>,
    ) -> Self {
        let router_address = router.address();
        let settlement_allowances =
            Box::new(AllowanceManager::new(web3, gpv2_settlement.address()));

        Self {
            inner: Arc::new(Inner {
                router,
                gpv2_settlement,
                allowances: Mutex::new(Allowances::empty(router_address)),
            }),
            pool_fetcher,
            settlement_allowances,
            base_tokens,
        }
    }

    /// Given a list of offchain orders returns the list of AMM liquidity to be considered
    pub async fn get_liquidity(
        &self,
        offchain_orders: &[LimitOrder],
        at_block: Block,
    ) -> Result<Vec<ConstantProductOrder>> {
        let mut pools = HashSet::new();

        for order in offchain_orders {
            let path_candidates = path_candidates(
                order.sell_token,
                order.buy_token,
                &self.base_tokens,
                DEFAULT_MAX_HOPS,
            );
            pools.extend(
                path_candidates
                    .iter()
                    .flat_map(|candidate| token_path_to_pair_path(candidate).into_iter()),
            );
        }

        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(pools, at_block).await? {
            tokens.insert(pool.tokens.get().0);
            tokens.insert(pool.tokens.get().1);

            result.push(ConstantProductOrder {
                tokens: pool.tokens,
                reserves: pool.reserves,
                fee: pool.fee,
                settlement_handling: self.inner.clone(),
            })
        }
        self.cache_allowances(tokens).await?;
        Ok(result)
    }

    async fn cache_allowances(&self, tokens: HashSet<H160>) -> Result<()> {
        let router = self.inner.router.address();
        let allowances = self
            .settlement_allowances
            .get_allowances(tokens, router)
            .await?;

        self.inner
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .extend(allowances)?;

        Ok(())
    }
}

impl Inner {
    fn settle(
        &self,
        (token_in, amount_in): (H160, U256),
        (token_out, amount_out): (H160, U256),
    ) -> (Approval, UniswapInteraction) {
        let amount_in_with_slippage = slippage::amount_plus_max_slippage(amount_in);
        let approval = self
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_in, amount_in_with_slippage);

        (
            approval,
            UniswapInteraction {
                router: self.router.clone(),
                settlement: self.gpv2_settlement.clone(),
                // Apply fixed slippage tolerance in case balances change between solution finding and mining
                amount_out,
                amount_in_max: amount_in_with_slippage,
                token_in,
                token_out,
            },
        )
    }
}

impl SettlementHandling<ConstantProductOrder> for Inner {
    // Creates the required interaction to convert the given input into output. Applies 0.1% slippage tolerance to the output.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let (approval, swap) = self.settle(execution.input, execution.output);
        encoder.append_to_execution_plan(approval);
        encoder.append_to_execution_plan(swap);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::dummy_contract;
    use std::collections::HashMap;

    impl Inner {
        fn new(allowances: HashMap<H160, U256>) -> Self {
            Self {
                router: dummy_contract!(IUniswapLikeRouter, H160::zero()),
                gpv2_settlement: dummy_contract!(GPv2Settlement, H160::zero()),
                allowances: Mutex::new(Allowances::new(H160::zero(), allowances)),
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
        let (approval, _) = inner.settle((token_a, 50.into()), (token_b, 100.into()));
        assert_eq!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_a, 99.into()), (token_b, 100.into()));
        assert_eq!(approval, Approval::AllowanceSufficient);

        // Allowance needed because of slippage
        let (approval, _) = inner.settle((token_a, 100.into()), (token_b, 100.into()));
        assert_ne!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_a, 150.into()), (token_b, 100.into()));
        assert_ne!(approval, Approval::AllowanceSufficient);

        // Token B below, equal, above
        let (approval, _) = inner.settle((token_b, 150.into()), (token_a, 100.into()));
        assert_eq!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_b, 199.into()), (token_a, 100.into()));
        assert_eq!(approval, Approval::AllowanceSufficient);

        // Allowance needed because of slippage
        let (approval, _) = inner.settle((token_b, 200.into()), (token_a, 100.into()));
        assert_ne!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_b, 250.into()), (token_a, 100.into()));
        assert_ne!(approval, Approval::AllowanceSufficient);

        // Untracked token
        let (approval, _) =
            inner.settle((H160::from_low_u64_be(3), 1.into()), (token_a, 100.into()));
        assert_ne!(approval, Approval::AllowanceSufficient);
    }
}
