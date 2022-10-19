//! Module containing implementation of the 1Inch solver.
//!
//! This simple solver will simply use the 1Inch API to get a quote for a
//! single GPv2 order and produce a settlement directly against 1Inch.

use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    interactions::allowances::{AllowanceManager, AllowanceManaging, ApprovalRequest},
    liquidity::{slippage::SlippageCalculator, LimitOrder},
    settlement::Settlement,
};
use anyhow::Result;
use contracts::GPv2Settlement;
use derivative::Derivative;
use ethcontract::Account;
use maplit::hashmap;
use model::order::OrderKind;
use primitive_types::H160;
use reqwest::{Client, Url};
use shared::{
    oneinch_api::{
        OneInchClient, OneInchClientImpl, OneInchError, ProtocolCache, Slippage, SwapQuery,
    },
    Web3,
};
use std::fmt::{self, Display, Formatter};

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
    protocol_cache: ProtocolCache,
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
            protocol_cache: ProtocolCache::default(),
            slippage_calculator,
            referrer_address,
        })
    }
}

impl OneInchSolver {
    /// Settles a single sell order against a 1Inch swap using the specified protocols and
    /// slippage.
    async fn settle_order_with_protocols_and_slippage(
        &self,
        order: LimitOrder,
        protocols: Option<Vec<String>>,
        slippage: Slippage,
    ) -> Result<Option<Settlement>, SettlementError> {
        debug_assert_eq!(
            order.kind,
            OrderKind::Sell,
            "only sell orders should be passed to try_settle_order"
        );

        let spender = self.client.get_spender().await?;
        // Fetching allowance before making the SwapQuery so that the Swap info is as recent as possible
        let approval = self
            .allowance_fetcher
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: spender.address,
                amount: order.sell_amount,
            })
            .await?;

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
        let swap = match self.client.get_swap(query).await {
            Ok(swap) => swap,
            Err(error) if error.is_insuffucient_liquidity() => {
                // This means the order cannot get matched which shouldn't be treated as an error.
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };

        if !execution_respects_order(&order, swap.from_token_amount, swap.to_token_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => swap.to_token_amount,
            order.buy_token => swap.from_token_amount,
        });

        settlement.with_liquidity(&order, order.sell_amount)?;

        settlement.encoder.append_to_execution_plan(approval);
        settlement.encoder.append_to_execution_plan(swap);

        Ok(Some(settlement))
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for OneInchSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<Settlement>, SettlementError> {
        if order.kind != OrderKind::Sell {
            // 1Inch only supports sell orders
            return Ok(None);
        }
        let protocols = self
            .protocol_cache
            .get_allowed_protocols(&self.disabled_protocols, self.client.as_ref())
            .await?;
        let slippage = Slippage::percentage(
            self.slippage_calculator
                .auction_context(auction)
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
        let retryable = matches!(&err, OneInchError::Api(err) if err.status_code == 500);
        SettlementError {
            inner: err.into(),
            retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interactions::allowances::{Approval, MockAllowanceManaging},
        liquidity::LimitOrder,
        solver::ExternalPrices,
        test::account,
    };
    use contracts::{GPv2Settlement, WETH9};
    use ethcontract::{Web3, H160, U256};
    use futures::FutureExt as _;
    use mockall::{predicate::*, Sequence};
    use model::order::{Order, OrderData, OrderKind};
    use shared::{
        conversions::U256Ext as _,
        dummy_contract,
        oneinch_api::{MockOneInchClient, Protocols, Spender, Swap},
        transport::create_env_test_transport,
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
            protocol_cache: ProtocolCache::default(),
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
                    &Auction::default()
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
            .returning(|_| Ok(Approval::AllowanceSufficient));

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

        let auction = Auction {
            external_prices: ExternalPrices::new(
                native_token,
                hashmap! {
                    buy_token => U256::exp10(18).to_big_rational(),
                },
            )
            .unwrap(),
            ..Default::default()
        };

        let result = solver
            .try_settle_order(order_passing_limit, &auction)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.clearing_prices(),
            // Note that prices are the inverted amounts. Another way to look at
            // it is if the swap requires 100 sell token to get only 99 buy
            // token, then the sell token is worth less (i.e. lower price) than
            // the buy token.
            &hashmap! {
                sell_token => 99.into(),
                buy_token => 100.into(),
            }
        );

        let result = solver
            .try_settle_order(order_violating_limit, &auction)
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
            .returning(|_| Ok(Approval::AllowanceSufficient));

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
                &Auction::default()
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
                Ok(Approval::Approve {
                    token: sell_token,
                    spender,
                })
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
            .returning(|_| Ok(Approval::AllowanceSufficient))
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
        let auction = Auction {
            external_prices: ExternalPrices::new(
                native_token,
                hashmap! {
                    buy_token => U256::exp10(18).to_big_rational(),
                },
            )
            .unwrap(),
            ..Default::default()
        };

        // On first run we have two main interactions (approve + swap)
        let result = solver
            .try_settle_order(order.clone(), &auction)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 2);

        // On second run we have only have one main interactions (swap)
        let result = solver
            .try_settle_order(order, &auction)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 1)
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

        println!("{:#?}", settlement);
    }
}
