//! Module containing implementation of the 1Inch solver.
//!
//! This simple solver will simply use the 1Inch API to get a quote for a
//! single GPv2 order and produce a settlement directly against 1Inch.

use {
    super::single_order_solver::{
        execution_respects_order,
        SettlementError,
        SingleOrderSettlement,
        SingleOrderSolving,
    },
    crate::{
        interactions::allowances::{AllowanceManager, AllowanceManaging, ApprovalRequest},
        liquidity::{slippage::SlippageCalculator, LimitOrder},
    },
    anyhow::Result,
    contracts::GPv2Settlement,
    derivative::Derivative,
    ethcontract::Account,
    model::order::OrderKind,
    primitive_types::H160,
    reqwest::{Client, Url},
    shared::{
        ethrpc::Web3,
        external_prices::ExternalPrices,
        interaction::Interaction,
        oneinch_api::{Cache, OneInchClient, OneInchClientImpl, OneInchError, Slippage, SwapQuery},
    },
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
};

/// A GPv2 solver that matches GP **sell** orders to direct 1Inch swaps.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct OneInchSolver {
    account: Account,
    settlement_contract: GPv2Settlement,
    disabled_protocols: Vec<String>,
    #[derivative(Debug = "ignore")]
    client: Box<dyn OneInchClient>,
    #[derivative(Debug = "ignore")]
    allowance_fetcher: Box<dyn AllowanceManaging>,
    cache: Cache,
    slippage_calculator: SlippageCalculator,
    referrer_address: Option<H160>,
}

impl OneInchSolver {
    /// Creates a new 1Inch solver with a list of disabled protocols.
    #[allow(clippy::too_many_arguments)]
    pub fn with_disabled_protocols(
        account: Account,
        web3: Web3,
        settlement_contract: GPv2Settlement,
        chain_id: u64,
        disabled_protocols: impl IntoIterator<Item = String>,
        client: Client,
        one_inch_url: Url,
        slippage_calculator: SlippageCalculator,
        referrer_address: Option<H160>,
    ) -> Result<Self> {
        let settlement_address = settlement_contract.address();
        Ok(Self {
            account,
            settlement_contract,
            disabled_protocols: disabled_protocols.into_iter().collect(),
            client: Box::new(OneInchClientImpl::new(one_inch_url, client, chain_id)?),
            allowance_fetcher: Box::new(AllowanceManager::new(web3, settlement_address)),
            cache: Cache::default(),
            slippage_calculator,
            referrer_address,
        })
    }
}

impl OneInchSolver {
    /// Settles a single sell order against a 1Inch swap using the specified
    /// protocols and slippage.
    async fn settle_order_with_protocols_and_slippage(
        &self,
        order: LimitOrder,
        protocols: Option<Vec<String>>,
        slippage: Slippage,
    ) -> Result<Option<SingleOrderSettlement>, SettlementError> {
        debug_assert_eq!(
            order.kind,
            OrderKind::Sell,
            "only sell orders should be passed to try_settle_order"
        );

        let mut interactions: Vec<Arc<dyn Interaction>> = Vec::new();

        let spender = self.cache.spender(self.client.as_ref()).await?;
        // Fetching allowance before making the SwapQuery so that the Swap info is as
        // recent as possible
        if let Some(approval) = self
            .allowance_fetcher
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: spender.address,
                amount: order.sell_amount,
            })
            .await?
        {
            interactions.push(Arc::new(approval));
        }

        let query = SwapQuery::with_default_options(
            order.sell_token,
            order.buy_token,
            order.sell_amount,
            self.settlement_contract.address(),
            protocols,
            slippage,
            self.referrer_address,
        );

        tracing::debug!("querying 1Inch swap api with {:?}", query);
        let swap = self.client.get_swap(query).await?;
        if !execution_respects_order(&order, swap.from_token_amount, swap.to_token_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let (sell_token_price, buy_token_price) = (swap.to_token_amount, swap.from_token_amount);
        interactions.push(Arc::new(swap));

        Ok(Some(SingleOrderSettlement {
            sell_token_price,
            buy_token_price,
            interactions,
            executed_amount: order.full_execution_amount(),
            order,
        }))
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for OneInchSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        external_prices: &ExternalPrices,
        _gas_price: f64,
    ) -> Result<Option<SingleOrderSettlement>, SettlementError> {
        if order.kind != OrderKind::Sell {
            // 1Inch only supports sell orders
            return Ok(None);
        }
        let protocols = self
            .cache
            .allowed_protocols(&self.disabled_protocols, self.client.as_ref())
            .await?;
        let slippage = Slippage::percentage(
            self.slippage_calculator
                .context(external_prices)
                .relative_for_order(&order)?
                .as_percentage(),
        )?;
        self.settle_order_with_protocols_and_slippage(order, protocols, slippage)
            .await
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "1Inch"
    }
}

impl Display for OneInchSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "OneInchSolver")
    }
}

impl From<OneInchError> for SettlementError {
    fn from(err: OneInchError) -> Self {
        match err {
            err if err.is_insuffucient_liquidity() => Self::Benign(err.into()),
            OneInchError::Api(err) if err.status_code == 429 => Self::RateLimited,
            OneInchError::Api(err) if err.status_code == 500 => {
                Self::Retryable(OneInchError::Api(err).into())
            }
            err => Self::Other(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            interactions::allowances::{Approval, MockAllowanceManaging},
            liquidity::LimitOrder,
            solver::ExternalPrices,
            test::account,
        },
        contracts::{dummy_contract, GPv2Settlement, WETH9},
        ethcontract::{Web3, H160, U256},
        futures::FutureExt as _,
        maplit::hashmap,
        mockall::{predicate::*, Sequence},
        model::order::{Order, OrderData, OrderKind},
        shared::{
            conversions::U256Ext,
            ethrpc::create_env_test_transport,
            oneinch_api::{MockOneInchClient, Protocols, Spender, Swap},
        },
    };

    fn dummy_solver(
        client: MockOneInchClient,
        allowance_fetcher: MockAllowanceManaging,
    ) -> OneInchSolver {
        let settlement_contract = dummy_contract!(GPv2Settlement, H160::zero());
        OneInchSolver {
            account: account(),
            settlement_contract,
            disabled_protocols: Vec::default(),
            client: Box::new(client),
            allowance_fetcher: Box::new(allowance_fetcher),
            cache: Cache::default(),
            slippage_calculator: SlippageCalculator::default(),
            referrer_address: None,
        }
    }

    #[tokio::test]
    async fn ignores_buy_orders() {
        assert!(
            dummy_solver(MockOneInchClient::new(), MockAllowanceManaging::new())
                .try_settle_order(
                    LimitOrder {
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    &Default::default(),
                    1.
                )
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_satisfies_limit_price() {
        let mut client = MockOneInchClient::new();
        let mut allowance_fetcher = MockAllowanceManaging::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let native_token = H160::from_low_u64_be(3);

        client.expect_get_spender().returning(|| {
            async {
                Ok(Spender {
                    address: H160::zero(),
                })
            }
            .boxed()
        });
        client.expect_get_swap().returning(|_| {
            async {
                Ok(Swap {
                    from_token_amount: 100.into(),
                    to_token_amount: 99.into(),
                    ..Default::default()
                })
            }
            .boxed()
        });

        allowance_fetcher
            .expect_get_approval()
            .returning(|_| Ok(None));

        let solver = dummy_solver(client, allowance_fetcher);

        let order_passing_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 90.into(),
            kind: model::order::OrderKind::Sell,
            ..Default::default()
        };
        let order_violating_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 110.into(),
            kind: model::order::OrderKind::Sell,
            ..Default::default()
        };

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {
                buy_token => U256::exp10(18).to_big_rational(),
            },
        )
        .unwrap();

        let result = solver
            .try_settle_order(order_passing_limit, &external_prices, 1.)
            .await
            .unwrap()
            .unwrap();
        // Note that prices are the inverted amounts. Another way to look at
        // it is if the swap requires 100 sell token to get only 99 buy
        // token, then the sell token is worth less (i.e. lower price) than
        // the buy token.
        assert_eq!(result.sell_token_price, 99.into());
        assert_eq!(result.buy_token_price, 100.into());

        let result = solver
            .try_settle_order(order_violating_limit, &external_prices, 1.)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn filters_disabled_protocols() {
        let mut client = MockOneInchClient::new();
        let mut allowance_fetcher = MockAllowanceManaging::new();

        allowance_fetcher
            .expect_get_approval()
            .returning(|_| Ok(None));

        client.expect_get_liquidity_sources().returning(|| {
            async {
                Ok(Protocols {
                    protocols: vec!["GoodProtocol".into(), "BadProtocol".into()],
                })
            }
            .boxed()
        });
        client.expect_get_spender().returning(|| {
            async {
                Ok(Spender {
                    address: H160::zero(),
                })
            }
            .boxed()
        });
        client.expect_get_swap().times(1).returning(|query| {
            async move {
                assert_eq!(query.quote.protocols, Some(vec!["GoodProtocol".into()]));
                Ok(Swap {
                    from_token_amount: 100.into(),
                    to_token_amount: 100.into(),
                    ..Default::default()
                })
            }
            .boxed()
        });

        let solver = OneInchSolver {
            disabled_protocols: vec!["BadProtocol".to_string(), "VeryBadProtocol".to_string()],
            ..dummy_solver(client, allowance_fetcher)
        };

        // Limit price violated. Actual assert is happening in `expect_get_swap()`
        assert!(solver
            .try_settle_order(
                LimitOrder {
                    kind: OrderKind::Sell,
                    buy_amount: U256::max_value(),
                    ..Default::default()
                },
                &Default::default(),
                1.
            )
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_sets_allowance_if_necessary() {
        let mut client = MockOneInchClient::new();
        let mut allowance_fetcher = MockAllowanceManaging::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let spender = H160::from_low_u64_be(3);

        client
            .expect_get_spender()
            .returning(move || async move { Ok(Spender { address: spender }) }.boxed());
        client.expect_get_swap().returning(|_| {
            async {
                Ok(Swap {
                    from_token_amount: 100.into(),
                    to_token_amount: 100.into(),
                    ..Default::default()
                })
            }
            .boxed()
        });

        // On first invocation no prior allowance, then max allowance set.
        let mut seq = Sequence::new();
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .with(eq(ApprovalRequest {
                token: sell_token,
                spender,
                amount: U256::from(100),
            }))
            .returning(move |_| {
                Ok(Some(Approval {
                    token: sell_token,
                    spender,
                }))
            })
            .in_sequence(&mut seq);
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .with(eq(ApprovalRequest {
                token: sell_token,
                spender,
                amount: U256::from(100),
            }))
            .returning(|_| Ok(None))
            .in_sequence(&mut seq);

        let solver = dummy_solver(client, allowance_fetcher);

        let order = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 90.into(),
            kind: OrderKind::Sell,
            ..Default::default()
        };

        let native_token = H160::from_low_u64_be(4);
        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {
                buy_token => U256::exp10(18).to_big_rational(),
            },
        )
        .unwrap();

        // On first run we have two main interactions (approve + swap)
        let result = solver
            .try_settle_order(order.clone(), &external_prices, 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.interactions.len(), 2);

        // On second run we have only have one main interactions (swap)
        let result = solver
            .try_settle_order(order, &external_prices, 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.interactions.len(), 1)
    }

    #[tokio::test]
    #[ignore]
    async fn solve_order_on_oneinch() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = testlib::tokens::GNO;

        let solver = OneInchSolver::with_disabled_protocols(
            account(),
            web3,
            settlement,
            chain_id,
            vec!["PMM1".to_string()],
            Client::new(),
            OneInchClientImpl::DEFAULT_URL.try_into().unwrap(),
            SlippageCalculator::default(),
            None,
        )
        .unwrap();
        let settlement = solver
            .settle_order_with_protocols_and_slippage(
                Order {
                    data: OrderData {
                        sell_token: weth.address(),
                        buy_token: gno,
                        sell_amount: 1_000_000_000_000_000_000u128.into(),
                        buy_amount: 1u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                None,
                Slippage::ONE_PERCENT,
            )
            .await
            .unwrap()
            .unwrap();

        println!("{settlement:#?}");
    }
}
