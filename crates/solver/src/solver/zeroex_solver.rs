//! Module containing implementation of the 0x solver.
//!
//! This solver will simply use the 0x API to get a quote for a
//! single GPv2 order and produce a settlement directly against 0x.
//!
//! Please be aware of the following subtlety for buy orders:
//! 0x's API is adding the defined slippage on the sellAmount of a buy order
//! and then returns the surplus in the buy amount to the user.
//! I.e. if the user defines a 5% slippage, they will sell 5% more, and receive
//! 5% more buy-tokens than ordered. Here is on example tx:
//! https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block=12735030&blockIndex=0&from=0xa6ddbd0de6b310819b49f680f65871bee85f517e&gas=8000000&gasPrice=0&value=0&contractAddress=0x3328f5f2cecaf00a2443082b657cedeaf70bfaef&rawFunctionInput=0x13d79a0b000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000003600000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000029143e200000000000000000000000000000000000000000000000000470de4df820000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000036416d81e590ff67370e4523b9cd3257aa0a853c000000000000000000000000000000000000000000000000000000000291f64800000000000000000000000000000000000000000000000000470de4df8200000000000000000000000000000000000000000000000000000000000060dc5839000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000003dc140000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000029143e2000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000410a7f27a6638cc9cdaba8266a15acef4cf7e1e1c9b9b2059391b7230b67bdfeb21f1d3aa45852f527a5040d3d7a190b92764a2c854f334b7eed579b390b85fd3f1b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000003800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000120000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b3000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25effffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000128d9627aa400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000002b220e100000000000000000000000000000000000000000000000000470de4df82000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2869584cd000000000000000000000000100000000000000000000000000000000000001100000000000000000000000000000000000000000000003239e38b8a60dc53b70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000&network=1
//! This behavior has the following risks: The additional sell tokens from the
//! slippage are not provided by the user, hence the additional tokens might be
//! not available in the settlement contract. For smaller amounts this is
//! unlikely, as we always charge the fees also in the sell token, though, the
//! fee's might not always be sufficient. This risk should be covered in a
//! future PR.
//!
//! Sell orders are unproblematic, especially, since the positive slippage is
//! handed back from 0x

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
    anyhow::{ensure, Context, Result},
    contracts::GPv2Settlement,
    ethcontract::{Account, H160},
    model::order::OrderKind,
    shared::{
        ethrpc::Web3,
        external_prices::ExternalPrices,
        zeroex_api::{Slippage, SwapQuery, ZeroExApi, ZeroExResponseError},
    },
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
};

/// A GPv2 solver that matches GP orders to direct 0x swaps.
pub struct ZeroExSolver {
    account: Account,
    api: Arc<dyn ZeroExApi>,
    allowance_fetcher: Box<dyn AllowanceManaging>,
    excluded_sources: Vec<String>,
    slippage_calculator: SlippageCalculator,
    settlement: H160,
    enable_rfqt: bool,
    enable_slippage_protection: bool,
}

/// Chain ID for Mainnet.
const MAINNET_CHAIN_ID: u64 = 1;

impl ZeroExSolver {
    pub fn new(
        account: Account,
        web3: Web3,
        settlement_contract: GPv2Settlement,
        chain_id: u64,
        api: Arc<dyn ZeroExApi>,
        excluded_sources: Vec<String>,
        slippage_calculator: SlippageCalculator,
    ) -> Result<Self> {
        ensure!(
            chain_id == MAINNET_CHAIN_ID,
            "0x solver only supported on Mainnet",
        );
        let allowance_fetcher = AllowanceManager::new(web3, settlement_contract.address());
        Ok(Self {
            account,
            allowance_fetcher: Box::new(allowance_fetcher),
            api,
            excluded_sources,
            slippage_calculator,
            settlement: settlement_contract.address(),
            enable_rfqt: false,
            enable_slippage_protection: false,
        })
    }

    pub fn with_rfqt(mut self, enable: bool) -> Self {
        self.enable_rfqt = enable;
        self
    }

    pub fn with_slippage_protection(mut self, enable: bool) -> Self {
        self.enable_slippage_protection = enable;
        self
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for ZeroExSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        external_prices: &ExternalPrices,
        _: f64,
    ) -> Result<Option<SingleOrderSettlement>, SettlementError> {
        let (buy_amount, sell_amount) = match order.kind {
            OrderKind::Buy => (Some(order.buy_amount), None),
            OrderKind::Sell => (None, Some(order.sell_amount)),
        };
        let query = SwapQuery {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount,
            buy_amount,
            slippage_percentage: Some(Slippage::new(
                self.slippage_calculator
                    .context(external_prices)
                    .relative_for_order(&order)?
                    .as_factor(),
            )),
            taker_address: Some(self.settlement),
            excluded_sources: self.excluded_sources.clone(),
            intent_on_filling: self.enable_rfqt,
            enable_slippage_protection: self.enable_slippage_protection,
        };
        let swap = match self.api.get_swap(query).await {
            Ok(swap) => swap,
            Err(ZeroExResponseError::InsufficientLiquidity) => {
                tracing::debug!("Couldn't get a quote due to insufficient liquidity");
                return Ok(None);
            }
            Err(err) => {
                return Err(err.into());
            }
        };

        if !execution_respects_order(&order, swap.price.sell_amount, swap.price.buy_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let approval = self
            .allowance_fetcher
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: swap.price.allowance_target,
                amount: swap.price.sell_amount,
            })
            .await
            .context("get_approval")?;

        let mut settlement = SingleOrderSettlement {
            sell_token_price: swap.price.buy_amount,
            buy_token_price: swap.price.sell_amount,
            interactions: Vec::new(),
            executed_amount: order.full_execution_amount(),
            order,
        };
        if let Some(approval) = &approval {
            settlement.interactions.push(Arc::new(*approval));
        }
        settlement.interactions.push(Arc::new(swap.clone()));
        Ok(Some(settlement))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "0x"
    }
}

impl From<ZeroExResponseError> for SettlementError {
    fn from(err: ZeroExResponseError) -> Self {
        match err {
            ZeroExResponseError::RateLimited => Self::RateLimited,
            err @ ZeroExResponseError::ServerError(_) => Self::Retryable(err.into()),
            err => Self::Other(err.into()),
        }
    }
}

impl Display for ZeroExSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ZeroExSolver")
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            interactions::allowances::{Approval, MockAllowanceManaging},
            liquidity::LimitOrder,
            test::account,
        },
        contracts::{GPv2Settlement, WETH9},
        ethcontract::{futures::FutureExt as _, Web3, H160, U256},
        mockall::{predicate::*, Sequence},
        model::order::{Order, OrderData, OrderKind},
        shared::{
            ethrpc::{create_env_test_transport, create_test_transport},
            zeroex_api::{DefaultZeroExApi, MockZeroExApi, PriceResponse, SwapResponse},
        },
    };

    #[tokio::test]
    #[ignore]
    async fn solve_sell_order_on_zeroex() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = testlib::tokens::GNO;

        let solver = ZeroExSolver::new(
            account(),
            web3,
            settlement,
            chain_id,
            Arc::new(DefaultZeroExApi::default()),
            Default::default(),
            SlippageCalculator::default(),
        )
        .unwrap();
        let settlement = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token: weth.address(),
                        buy_token: gno,
                        sell_amount: 1_000_000_000_000_000_000u128.into(),
                        buy_amount: 2u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Default::default(),
                1.,
            )
            .await
            .unwrap();

        println!("{settlement:#?}");
    }

    #[tokio::test]
    #[ignore]
    async fn solve_buy_order_on_zeroex() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = testlib::tokens::GNO;

        let solver = ZeroExSolver::new(
            account(),
            web3,
            settlement,
            chain_id,
            Arc::new(DefaultZeroExApi::default()),
            Default::default(),
            SlippageCalculator::default(),
        )
        .unwrap();
        let settlement = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token: weth.address(),
                        buy_token: gno,
                        sell_amount: 1_000_000_000_000_000_000u128.into(),
                        buy_amount: 1_000_000_000_000_000_000u128.into(),
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Default::default(),
                1.,
            )
            .await
            .unwrap();

        println!("{settlement:#?}");
    }

    #[tokio::test]
    async fn test_satisfies_limit_price_for_orders() {
        let mut client = MockZeroExApi::new();
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        let allowance_target = shared::addr!("def1c0ded9bec7f1a1670819833240f027b25eff");
        client.expect_get_swap().returning(move |_| {
            async move {
                Ok(SwapResponse {
                    price: PriceResponse {
                        sell_amount: U256::from_dec_str("100").unwrap(),
                        buy_amount: U256::from_dec_str("91").unwrap(),
                        allowance_target,
                        price: 0.91_f64,
                        estimated_gas: Default::default(),
                    },
                    to: shared::addr!("0000000000000000000000000000000000000000"),
                    data: hex::decode("00").unwrap(),
                    value: U256::from_dec_str("0").unwrap(),
                })
            }
            .boxed()
        });

        allowance_fetcher
            .expect_get_approval()
            .times(2)
            .with(eq(ApprovalRequest {
                token: sell_token,
                spender: allowance_target,
                amount: U256::from(100),
            }))
            .returning(move |_| {
                Ok(Some(Approval {
                    token: sell_token,
                    spender: allowance_target,
                }))
            });

        let solver = ZeroExSolver {
            account: account(),
            api: Arc::new(client),
            allowance_fetcher,
            excluded_sources: Default::default(),
            slippage_calculator: Default::default(),
            settlement: testlib::protocol::SETTLEMENT,
            enable_rfqt: false,
            enable_slippage_protection: false,
        };

        let buy_order_passing_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 101.into(),
            buy_amount: 91.into(),
            kind: model::order::OrderKind::Buy,
            ..Default::default()
        };
        let buy_order_violating_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 99.into(),
            buy_amount: 91.into(),
            kind: model::order::OrderKind::Buy,
            ..Default::default()
        };
        let sell_order_passing_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 90.into(),
            kind: model::order::OrderKind::Sell,
            ..Default::default()
        };
        let sell_order_violating_limit = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 110.into(),
            kind: model::order::OrderKind::Sell,
            ..Default::default()
        };

        let result = solver
            .try_settle_order(sell_order_passing_limit, &Default::default(), 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.sell_token_price, 91.into());
        assert_eq!(result.buy_token_price, 100.into());

        let result = solver
            .try_settle_order(sell_order_violating_limit, &Default::default(), 1.)
            .await
            .unwrap();
        assert!(result.is_none());

        let result = solver
            .try_settle_order(buy_order_passing_limit, &Default::default(), 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.sell_token_price, 91.into());
        assert_eq!(result.buy_token_price, 100.into());

        let result = solver
            .try_settle_order(buy_order_violating_limit, &Default::default(), 1.)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn returns_error_on_non_mainnet() {
        let web3 = Web3::new(create_test_transport(
            &std::env::var("NODE_URL_RINKEBY").unwrap(),
        ));
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        assert!(ZeroExSolver::new(
            account(),
            web3,
            settlement,
            chain_id,
            Arc::new(DefaultZeroExApi::default()),
            Default::default(),
            SlippageCalculator::default(),
        )
        .is_err())
    }

    #[tokio::test]
    async fn test_sets_allowance_if_necessary() {
        let mut client = MockZeroExApi::new();
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        let allowance_target = shared::addr!("def1c0ded9bec7f1a1670819833240f027b25eff");
        client.expect_get_swap().returning(move |_| {
            async move {
                Ok(SwapResponse {
                    price: PriceResponse {
                        sell_amount: U256::from_dec_str("100").unwrap(),
                        buy_amount: U256::from_dec_str("91").unwrap(),
                        allowance_target,
                        price: 13.121_002_575_170_278_f64,
                        estimated_gas: Default::default(),
                    },
                    to: shared::addr!("0000000000000000000000000000000000000000"),
                    data: hex::decode("").unwrap(),
                    value: U256::from_dec_str("0").unwrap(),
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
                spender: allowance_target,
                amount: U256::from(100),
            }))
            .returning(move |_| {
                Ok(Some(Approval {
                    token: sell_token,
                    spender: allowance_target,
                }))
            })
            .in_sequence(&mut seq);
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .returning(|_| Ok(None))
            .in_sequence(&mut seq);

        let solver = ZeroExSolver {
            account: account(),
            api: Arc::new(client),
            allowance_fetcher,
            excluded_sources: Default::default(),
            slippage_calculator: Default::default(),
            settlement: testlib::protocol::SETTLEMENT,
            enable_rfqt: false,
            enable_slippage_protection: false,
        };

        let order = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 90.into(),
            ..Default::default()
        };

        // On first run we have two main interactions (approve + swap)
        let result = solver
            .try_settle_order(order.clone(), &Default::default(), 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.interactions.len(), 2);

        // On second run we have only have one main interactions (swap)
        let result = solver
            .try_settle_order(order, &Default::default(), 1.)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.interactions.len(), 1)
    }
}
