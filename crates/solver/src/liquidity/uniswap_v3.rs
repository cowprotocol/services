use {
    super::{AmmOrderExecution, ConcentratedLiquidity, SettlementHandling},
    crate::{
        interactions::{
            UniswapV3Interaction,
            allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval},
        },
        liquidity::Liquidity,
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    alloy::primitives::Address,
    anyhow::{Context, Result, ensure},
    contracts::alloy::UniswapV3SwapRouterV2::IV3SwapRouter::ExactOutputSingleParams,
    model::TokenPair,
    num::{CheckedMul, rational::Ratio},
    shared::{
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        sources::uniswap_v3::pool_fetching::PoolFetching,
        web3::Web3,
    },
    std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    },
    tracing::instrument,
};

pub struct UniswapV3Liquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
    settlement_allowances: Box<dyn AllowanceManaging>,
}
pub struct Inner {
    pub router: Address,
    gpv2_settlement: Address,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement
    // contract
    allowances: Mutex<Allowances>,
}

pub struct UniswapV3SettlementHandler {
    pub inner: Arc<Inner>,
    pub fee: u32,
}

impl UniswapV3SettlementHandler {
    pub fn new(
        router: Address,
        gpv2_settlement: Address,
        allowances: Mutex<Allowances>,
        fee: Ratio<u32>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                router,
                gpv2_settlement,
                allowances,
            }),
            fee: ratio_to_u32(fee).unwrap(),
        }
    }
}

/// Highly corelated to Uniswap V3 only.
/// Converts:
/// 1% fee to 10000
/// 0.3% fee to 3000
/// 0.05% to 500
/// 0.01% to 100
fn ratio_to_u32(ratio: Ratio<u32>) -> Result<u32> {
    Ok(ratio
        .checked_mul(&Ratio::new(1_000_000, 1))
        .context("failed multiplication")?
        .to_integer())
}

impl UniswapV3Liquidity {
    pub fn new(
        router: Address,
        gpv2_settlement: Address,
        web3: Web3,
        pool_fetcher: Arc<dyn PoolFetching>,
    ) -> Self {
        let settlement_allowances = Box::new(AllowanceManager::new(web3, gpv2_settlement));
        Self {
            inner: Arc::new(Inner {
                router,
                gpv2_settlement,
                allowances: Mutex::new(Allowances::empty(router)),
            }),
            pool_fetcher,
            settlement_allowances,
        }
    }

    async fn cache_allowances(&self, tokens: HashSet<Address>) -> Result<()> {
        let router = self.inner.router;
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
impl LiquidityCollecting for UniswapV3Liquidity {
    /// Given a list of offchain orders returns the list of AMM liquidity to be
    /// considered
    #[instrument(name = "uniswap_v3_liquidity", skip_all)]
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        block: Block,
    ) -> Result<Vec<Liquidity>> {
        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(&pairs, block).await? {
            ensure!(
                pool.tokens.len() == 2,
                "two tokens required for uniswap v3 pools"
            );
            let token_pair =
                TokenPair::new(pool.tokens[0].id, pool.tokens[1].id).context("cant create pair")?;

            tokens.insert(pool.tokens[0].id);
            tokens.insert(pool.tokens[1].id);

            result.push(Liquidity::Concentrated(ConcentratedLiquidity {
                tokens: token_pair,
                settlement_handling: Arc::new(UniswapV3SettlementHandler {
                    inner: self.inner.clone(),
                    fee: ratio_to_u32(pool.state.fee)?,
                }),
                pool,
            }))
        }
        self.cache_allowances(tokens).await?;
        Ok(result)
    }
}

impl UniswapV3SettlementHandler {
    pub fn settle(
        &self,
        token_amount_in_max: TokenAmount,
        token_amount_out: TokenAmount,
    ) -> (Option<Approval>, UniswapV3Interaction) {
        let approval = self
            .inner
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_amount_in_max.clone());

        let fee = self.fee.try_into().expect("fee < (1 << 24)");

        (
            approval,
            UniswapV3Interaction {
                router: self.inner.router,
                params: ExactOutputSingleParams {
                    tokenIn: token_amount_in_max.token,
                    tokenOut: token_amount_out.token,
                    fee,
                    recipient: self.inner.gpv2_settlement,
                    amountOut: token_amount_out.amount,
                    amountInMaximum: token_amount_in_max.amount,
                    sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
                },
            },
        )
    }
}

impl SettlementHandling<ConcentratedLiquidity> for UniswapV3SettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // Creates the required interaction to convert the given input into output.
    // Assumes slippage is already applied to the `input_max` field.
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        let (approval, swap) = self.settle(execution.input_max, execution.output);
        if let Some(approval) = approval {
            encoder.append_to_execution_plan_internalizable(
                Arc::new(approval),
                execution.internalizable,
            );
        }
        encoder.append_to_execution_plan_internalizable(Arc::new(swap), execution.internalizable);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::U256, num::rational::Ratio, std::collections::HashMap};

    impl UniswapV3SettlementHandler {
        fn new_dummy(allowances: HashMap<Address, U256>, fee: u32) -> Self {
            Self {
                inner: Arc::new(Inner {
                    router: Default::default(),
                    gpv2_settlement: Default::default(),
                    allowances: Mutex::new(Allowances::new(Address::ZERO, allowances)),
                }),
                fee,
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

        let settlement_handler = UniswapV3SettlementHandler::new_dummy(allowances, 10);

        // Token A below, equal, above
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_a,
                amount: U256::from(50),
            },
            TokenAmount {
                token: token_b,
                amount: U256::from(100),
            },
        );
        assert_eq!(approval, None);

        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_a,
                amount: U256::from(99),
            },
            TokenAmount {
                token: token_b,
                amount: U256::from(100),
            },
        );
        assert_eq!(approval, None);

        // Token B below, equal, above
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_b,
                amount: U256::from(150),
            },
            TokenAmount {
                token: token_a,
                amount: U256::from(100),
            },
        );
        assert_eq!(approval, None);

        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_b,
                amount: U256::from(199),
            },
            TokenAmount {
                token: token_a,
                amount: U256::from(100),
            },
        );
        assert_eq!(approval, None);

        // Untracked token
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: Address::with_last_byte(3),
                amount: U256::from(1),
            },
            TokenAmount {
                token: token_a,
                amount: U256::from(100),
            },
        );
        assert_ne!(approval, None);
    }

    #[test]
    fn test_ratio_to_u32() {
        let fee_1 = Ratio::<u32>::new(1, 100);
        let fee_2 = Ratio::<u32>::new(3, 1000);
        let fee_3 = Ratio::<u32>::new(5, 10000);
        let fee_4 = Ratio::<u32>::new(1, 10000);

        assert_eq!(ratio_to_u32(fee_1).unwrap(), 10000);
        assert_eq!(ratio_to_u32(fee_2).unwrap(), 3000);
        assert_eq!(ratio_to_u32(fee_3).unwrap(), 500);
        assert_eq!(ratio_to_u32(fee_4).unwrap(), 100);
    }
}
