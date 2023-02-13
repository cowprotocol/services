use {
    super::{AmmOrderExecution, ConcentratedLiquidity, SettlementHandling},
    crate::{
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval},
            ExactOutputSingleParams,
            UniswapV3Interaction,
        },
        liquidity::Liquidity,
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    anyhow::{ensure, Context, Result},
    contracts::{GPv2Settlement, UniswapV3SwapRouter},
    model::TokenPair,
    num::{rational::Ratio, CheckedMul},
    primitive_types::{H160, U256},
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        sources::uniswap_v3::pool_fetching::PoolFetching,
    },
    std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    },
};

// 1h timeout for Uniswap V3 interactions
const TIMEOUT: u32 = 3600;

pub struct UniswapV3Liquidity {
    inner: Arc<Inner>,
    pool_fetcher: Arc<dyn PoolFetching>,
    settlement_allowances: Box<dyn AllowanceManaging>,
}
pub struct Inner {
    router: UniswapV3SwapRouter,
    gpv2_settlement: GPv2Settlement,
    // Mapping of how much allowance the router has per token to spend on behalf of the settlement
    // contract
    allowances: Mutex<Allowances>,
}

pub struct UniswapV3SettlementHandler {
    inner: Arc<Inner>,
    fee: Option<u32>,
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
        router: UniswapV3SwapRouter,
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
impl LiquidityCollecting for UniswapV3Liquidity {
    /// Given a list of offchain orders returns the list of AMM liquidity to be
    /// considered
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
                    fee: Some(ratio_to_u32(pool.state.fee)?),
                }),
                pool,
            }))
        }
        self.cache_allowances(tokens).await?;
        Ok(result)
    }
}

impl UniswapV3SettlementHandler {
    fn settle(
        &self,
        token_amount_in_max: TokenAmount,
        token_amount_out: TokenAmount,
        fee: u32,
    ) -> (Option<Approval>, UniswapV3Interaction) {
        let approval = self
            .inner
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_amount_in_max.clone());

        (
            approval,
            UniswapV3Interaction {
                router: self.inner.router.clone(),
                params: ExactOutputSingleParams {
                    token_amount_in_max,
                    token_amount_out,
                    fee,
                    recipient: self.inner.gpv2_settlement.address(),
                    deadline: {
                        model::time::now_in_epoch_seconds()
                            .saturating_add(TIMEOUT)
                            .into()
                    },
                    sqrt_price_limit_x96: U256::zero(),
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
        let (approval, swap) = self.settle(
            execution.input_max,
            execution.output,
            self.fee.context("missing fee")?,
        );
        if let Some(approval) = approval {
            encoder.append_to_execution_plan_internalizable(approval, execution.internalizable);
        }
        encoder.append_to_execution_plan_internalizable(swap, execution.internalizable);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, num::rational::Ratio, shared::dummy_contract, std::collections::HashMap};

    impl UniswapV3SettlementHandler {
        fn new_dummy(allowances: HashMap<H160, U256>) -> Self {
            Self {
                inner: Arc::new(Inner {
                    router: dummy_contract!(UniswapV3SwapRouter, H160::zero()),
                    gpv2_settlement: dummy_contract!(GPv2Settlement, H160::zero()),
                    allowances: Mutex::new(Allowances::new(H160::zero(), allowances)),
                }),
                fee: None,
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

        let settlement_handler = UniswapV3SettlementHandler::new_dummy(allowances);

        // Token A below, equal, above
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_a,
                amount: 50.into(),
            },
            TokenAmount {
                token: token_b,
                amount: 100.into(),
            },
            10,
        );
        assert_eq!(approval, None);

        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_a,
                amount: 99.into(),
            },
            TokenAmount {
                token: token_b,
                amount: 100.into(),
            },
            10,
        );
        assert_eq!(approval, None);

        // Token B below, equal, above
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_b,
                amount: 150.into(),
            },
            TokenAmount {
                token: token_a,
                amount: 100.into(),
            },
            10,
        );
        assert_eq!(approval, None);

        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: token_b,
                amount: 199.into(),
            },
            TokenAmount {
                token: token_a,
                amount: 100.into(),
            },
            10,
        );
        assert_eq!(approval, None);

        // Untracked token
        let (approval, _) = settlement_handler.settle(
            TokenAmount {
                token: H160::from_low_u64_be(3),
                amount: 1.into(),
            },
            TokenAmount {
                token: token_a,
                amount: 100.into(),
            },
            10,
        );
        assert_ne!(approval, None);
    }

    #[test]
    fn test_encode() {
        let settlement_handler = UniswapV3SettlementHandler::new_dummy(Default::default());
        let execution = AmmOrderExecution {
            input_max: TokenAmount {
                token: H160::default(),
                amount: U256::zero(),
            },
            output: TokenAmount {
                token: H160::default(),
                amount: U256::zero(),
            },
            internalizable: false,
        };
        let mut encoder = SettlementEncoder::new(Default::default());
        let encoded = settlement_handler
            .encode(execution, &mut encoder)
            .unwrap_err();
        assert!(encoded.to_string() == "missing fee");
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
