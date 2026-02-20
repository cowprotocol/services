use {
    super::{AmmOrderExecution, ConstantProductOrder, SettlementHandling},
    crate::{
        interactions::UniswapInteraction,
        liquidity::Liquidity,
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    alloy::primitives::Address,
    anyhow::Result,
    model::TokenPair,
    shared::{
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        sources::uniswap_v2::pool_fetching::PoolFetching,
    },
    std::{collections::HashSet, sync::Arc},
    tracing::instrument,
};

pub struct UniswapLikeLiquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
}

pub struct Inner {
    router: Address,
    gpv2_settlement: Address,
}

impl UniswapLikeLiquidity {
    pub fn new(
        router: Address,
        gpv2_settlement: Address,
        pool_fetcher: Arc<dyn PoolFetching>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                router,
                gpv2_settlement,
            }),
            pool_fetcher,
        }
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for UniswapLikeLiquidity {
    /// Given a list of offchain orders returns the list of AMM liquidity to be
    /// considered
    #[instrument(name = "uniswap_like_liquidity", skip_all)]
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
        Ok(result)
    }
}

impl Inner {
    pub fn new(router: Address, gpv2_settlement: Address) -> Self {
        Inner {
            router,
            gpv2_settlement,
        }
    }

    pub fn settle(
        &self,
        token_amount_in_max: TokenAmount,
        token_amount_out: TokenAmount,
    ) -> UniswapInteraction {
        UniswapInteraction {
            router: self.router,
            settlement: self.gpv2_settlement,
            amount_out: token_amount_out.amount,
            amount_in_max: token_amount_in_max.amount,
            token_in: token_amount_in_max.token,
            token_out: token_amount_out.token,
        }
    }

    pub fn router(&self) -> Address {
        self.router
    }
}

impl SettlementHandling<ConstantProductOrder> for Inner {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // Creates the required interaction to convert the given input into output.
    // Assumes slippage is already applied to `input_max`.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let swap = Arc::new(self.settle(execution.input_max, execution.output));
        encoder.append_to_execution_plan_internalizable(swap, execution.internalizable);
        Ok(())
    }
}

/*
#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{Address, U256},
        std::collections::HashMap,
    };

    impl Inner {
        fn new_dummy(allowances: HashMap<Address, U256>) -> Self {
            Self {
                router: Address::default(),
                gpv2_settlement: Default::default(),
                allowances: Mutex::new(Allowances::new(Address::ZERO, allowances)),
            }
        }
    }

    #[test]
    fn test_should_set_allowance() {
        let token_a = Address::with_last_byte(1);
        let token_b = Address::with_last_byte(2);
        let allowances = maplit::hashmap! {
            token_a => U256::from(100),
            token_b => U256::from(200),
        };

        let inner = Inner::new_dummy(allowances);

        // Token A below, equal, above
        let (approval, _) = inner.settle(
            TokenAmount::new(token_a, U256::from(50)),
            TokenAmount::new(token_b, U256::from(100)),
        );
        assert_eq!(approval, None);

        let (approval, _) = inner.settle(
            TokenAmount::new(token_a, U256::from(99)),
            TokenAmount::new(token_b, U256::from(100)),
        );
        assert_eq!(approval, None);

        // Token B below, equal, above
        let (approval, _) = inner.settle(
            TokenAmount::new(token_b, U256::from(150)),
            TokenAmount::new(token_a, U256::from(100)),
        );
        assert_eq!(approval, None);

        let (approval, _) = inner.settle(
            TokenAmount::new(token_b, U256::from(199)),
            TokenAmount::new(token_a, U256::from(100)),
        );
        assert_eq!(approval, None);

        // Untracked token
        let (approval, _) = inner.settle(
            TokenAmount::new(Address::with_last_byte(3), U256::from(1)),
            TokenAmount::new(token_a, U256::from(100)),
        );
        assert_ne!(approval, None);
    }
}
*/
