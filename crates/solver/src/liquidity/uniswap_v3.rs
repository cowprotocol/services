use super::{slippage, AmmOrderExecution, ConcentratedLiquidity, LimitOrder, SettlementHandling};
use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval},
        ExactOutputSingleParams, UniswapV3Interaction,
    },
    settlement::SettlementEncoder,
};
use anyhow::{ensure, Context, Result};
use contracts::{GPv2Settlement, UniswapV3SwapRouter};
use model::TokenPair;
use primitive_types::{H160, U256};
use shared::{
    baseline_solver::BaseTokens, recent_block_cache::Block,
    sources::uniswap_v3::pool_fetching::PoolFetching, Web3,
};
use std::collections::HashSet;
use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
pub struct UniswapV3Liquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
    settlement_allowances: Box<dyn AllowanceManaging>,
    base_tokens: Arc<BaseTokens>,
}
pub struct Inner {
    router: UniswapV3SwapRouter,
    gpv2_settlement: GPv2Settlement,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement contract
    allowances: Mutex<Allowances>,
}

#[cfg(test)]
impl Inner {
    pub fn new(
        router: UniswapV3SwapRouter,
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

impl UniswapV3Liquidity {
    pub fn new(
        router: UniswapV3SwapRouter,
        gpv2_settlement: GPv2Settlement,
        base_tokens: Arc<BaseTokens>,
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
        _at_block: Block,
    ) -> Result<Vec<ConcentratedLiquidity>> {
        let pairs = self.base_tokens.relevant_pairs(
            &mut offchain_orders
                .iter()
                .flat_map(|order| TokenPair::new(order.buy_token, order.sell_token)),
        );

        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(&pairs).await? {
            ensure!(
                pool.tokens.len() == 2,
                "two tokens required for uniswap v3 pools"
            );
            let token_pair =
                TokenPair::new(pool.tokens[0].id, pool.tokens[1].id).context("cant create pair")?;

            tokens.insert(pool.tokens[0].id);
            tokens.insert(pool.tokens[1].id);

            result.push(ConcentratedLiquidity {
                tokens: token_pair,
                pool,
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
        fee: u32,
    ) -> (Approval, UniswapV3Interaction) {
        let amount_in_with_slippage = slippage::amount_plus_max_slippage(amount_in);
        let approval = self
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_in, amount_in_with_slippage);

        (
            approval,
            UniswapV3Interaction {
                router: self.router.clone(),
                //settlement: self.gpv2_settlement.clone(),
                params: ExactOutputSingleParams {
                    token_in,
                    token_out,
                    fee,
                    recipient: self.gpv2_settlement.address(),
                    deadline: {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        (now as u64).into()
                    },
                    amount_out,
                    amount_in_max: amount_in_with_slippage,
                    sqrt_price_limit_x96: U256::zero(),
                },
            },
        )
    }
}

impl SettlementHandling<ConcentratedLiquidity> for Inner {
    // Creates the required interaction to convert the given input into output. Applies 0.1% slippage tolerance to the output.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let (approval, swap) = self.settle(
            execution.input,
            execution.output,
            execution.fee.context("missing fee")?,
        );
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
        fn new_dummy(allowances: HashMap<H160, U256>) -> Self {
            Self {
                router: dummy_contract!(UniswapV3SwapRouter, H160::zero()),
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
        let (approval, _) = inner.settle((token_a, 50.into()), (token_b, 100.into()), 10);
        assert_eq!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_a, 99.into()), (token_b, 100.into()), 10);
        assert_eq!(approval, Approval::AllowanceSufficient);

        // Allowance needed because of slippage
        let (approval, _) = inner.settle((token_a, 100.into()), (token_b, 100.into()), 10);
        assert_ne!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_a, 150.into()), (token_b, 100.into()), 10);
        assert_ne!(approval, Approval::AllowanceSufficient);

        // Token B below, equal, above
        let (approval, _) = inner.settle((token_b, 150.into()), (token_a, 100.into()), 10);
        assert_eq!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_b, 199.into()), (token_a, 100.into()), 10);
        assert_eq!(approval, Approval::AllowanceSufficient);

        // Allowance needed because of slippage
        let (approval, _) = inner.settle((token_b, 200.into()), (token_a, 100.into()), 10);
        assert_ne!(approval, Approval::AllowanceSufficient);

        let (approval, _) = inner.settle((token_b, 250.into()), (token_a, 100.into()), 10);
        assert_ne!(approval, Approval::AllowanceSufficient);

        // Untracked token
        let (approval, _) = inner.settle(
            (H160::from_low_u64_be(3), 1.into()),
            (token_a, 100.into()),
            10,
        );
        assert_ne!(approval, Approval::AllowanceSufficient);
    }
}
