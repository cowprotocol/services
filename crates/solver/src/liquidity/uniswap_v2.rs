use super::{AmmOrderExecution, ConstantProductOrder, SettlementHandling};
use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval},
        UniswapInteraction,
    },
    liquidity::Liquidity,
    liquidity_collector::LiquidityCollecting,
    settlement::SettlementEncoder,
};
use anyhow::Result;
use contracts::{GPv2Settlement, IUniswapLikeRouter};
use model::TokenPair;
use primitive_types::{H160, U256};
use shared::{
    ethrpc::Web3, recent_block_cache::Block, sources::uniswap_v2::pool_fetching::PoolFetching,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

pub struct UniswapLikeLiquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
    settlement_allowances: Box<dyn AllowanceManaging>,
}

pub struct Inner {
    router: IUniswapLikeRouter,
    gpv2_settlement: GPv2Settlement,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement contract
    allowances: Mutex<Allowances>,
}

#[cfg(test)]
impl Inner {
    pub fn new(
        router: IUniswapLikeRouter,
        gpv2_settlement: GPv2Settlement,
        allowances: Mutex<Allowances>,
    ) -> Self {
        Inner {
            router,
            gpv2_settlement,
            allowances,
        }
    }
}

impl UniswapLikeLiquidity {
    pub fn new(
        router: IUniswapLikeRouter,
        gpv2_settlement: GPv2Settlement,
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
        }
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

#[async_trait::async_trait]
impl LiquidityCollecting for UniswapLikeLiquidity {
    /// Given a list of offchain orders returns the list of AMM liquidity to be considered
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(pairs, at_block).await? {
            tokens.insert(pool.tokens.get().0);
            tokens.insert(pool.tokens.get().1);

            result.push(Liquidity::ConstantProduct(ConstantProductOrder {
                address: pool.address,
                tokens: pool.tokens,
                reserves: pool.reserves,
                fee: pool.fee,
                settlement_handling: self.inner.clone(),
            }))
        }
        self.cache_allowances(tokens).await?;
        Ok(result)
    }
}

impl Inner {
    fn settle(
        &self,
        (token_in, amount_in_max): (H160, U256),
        (token_out, amount_out): (H160, U256),
    ) -> (Option<Approval>, UniswapInteraction) {
        let approval = self
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_in, amount_in_max);

        (
            approval,
            UniswapInteraction {
                router: self.router.clone(),
                settlement: self.gpv2_settlement.clone(),
                amount_out,
                amount_in_max,
                token_in,
                token_out,
            },
        )
    }

    pub fn router(&self) -> &IUniswapLikeRouter {
        &self.router
    }
}

impl SettlementHandling<ConstantProductOrder> for Inner {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // Creates the required interaction to convert the given input into output. Assumes slippage is
    // already applied to `input_max`.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let (approval, swap) = self.settle(execution.input_max, execution.output);
        if let Some(approval) = approval {
            encoder.append_to_execution_plan_internalizable(approval, execution.internalizable);
        }
        encoder.append_to_execution_plan_internalizable(swap, execution.internalizable);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::dummy_contract;
    use std::collections::HashMap;

    impl Inner {
        fn new_dummy(allowances: HashMap<H160, U256>) -> Self {
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

        let inner = Inner::new_dummy(allowances);

        // Token A below, equal, above
        let (approval, _) = inner.settle((token_a, 50.into()), (token_b, 100.into()));
        assert_eq!(approval, None);

        let (approval, _) = inner.settle((token_a, 99.into()), (token_b, 100.into()));
        assert_eq!(approval, None);

        // Token B below, equal, above
        let (approval, _) = inner.settle((token_b, 150.into()), (token_a, 100.into()));
        assert_eq!(approval, None);

        let (approval, _) = inner.settle((token_b, 199.into()), (token_a, 100.into()));
        assert_eq!(approval, None);

        // Untracked token
        let (approval, _) =
            inner.settle((H160::from_low_u64_be(3), 1.into()), (token_a, 100.into()));
        assert_ne!(approval, None);
    }
}
