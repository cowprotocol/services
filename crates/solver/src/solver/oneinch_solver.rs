//! Module containing implementation of the 1Inch solver.
//!
//! This simple solver will simply use the 1Inch API to get a quote for a
//! single GPv2 order and produce a settlement directly against 1Inch.

use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    encoding::EncodedInteraction,
    interactions::allowances::{AllowanceManager, AllowanceManaging, ApprovalRequest},
    liquidity::LimitOrder,
    settlement::{Interaction, Settlement},
};
use anyhow::{anyhow, Result};
use contracts::GPv2Settlement;
use derivative::Derivative;
use ethcontract::{Account, Bytes};
use maplit::hashmap;
use model::order::OrderKind;
use num::{BigRational, FromPrimitive, ToPrimitive};
use primitive_types::U256;
use reqwest::Client;
use reqwest::Url;
use shared::conversions::U256Ext;
use shared::oneinch_api::{
    OneInchClient, OneInchClientImpl, ProtocolCache, RestError, RestResponse, Swap, SwapQuery,
};
use shared::solver_utils::Slippage;
use shared::Web3;
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
    oneinch_slippage_bps: u32,
    /// how much slippage in wei we allow per trade
    max_slippage_in_wei: U256,
}

impl From<RestError> for SettlementError {
    fn from(error: RestError) -> Self {
        SettlementError {
            inner: anyhow!(error.description),
            retryable: matches!(error.status_code, 500),
        }
    }
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
        oneinch_slippage_bps: u32,
        max_slippage_in_wei: U256,
    ) -> Result<Self> {
        let settlement_address = settlement_contract.address();
        Ok(Self {
            account,
            settlement_contract,
            disabled_protocols: disabled_protocols.into_iter().collect(),
            client: Box::new(OneInchClientImpl::new(one_inch_url, client, chain_id)?),
            allowance_fetcher: Box::new(AllowanceManager::new(web3, settlement_address)),
            protocol_cache: ProtocolCache::default(),
            oneinch_slippage_bps,
            max_slippage_in_wei,
        })
    }
}

impl OneInchSolver {
    /// Computes the max slippage we are willing to use for a given trade to limit the absolute
    /// slippage to a configured upper limit in terms of wei. Because 1Inch keeps positive
    /// slippage, always applying a default slippage would otherwise become very costly for huge
    /// orders.
    fn compute_max_slippage(
        external_buy_token_price_in_wei: &BigRational,
        buy_amount: &U256,
        default_slippage_bps: u32,
        max_slippage_in_wei: &U256,
    ) -> Result<Slippage> {
        let max_slippage_in_buy_token =
            max_slippage_in_wei.to_big_rational() / external_buy_token_price_in_wei;

        let max_slippage_percent_respecting_wei_limit =
            max_slippage_in_buy_token / buy_amount.to_big_rational();

        let max_slippage_bps_respecting_wei_limit =
            max_slippage_percent_respecting_wei_limit * BigRational::from_u128(10_000).unwrap();

        let final_slippage_bps = std::cmp::min(
            max_slippage_bps_respecting_wei_limit
                .to_u32()
                // if the wei based slippage is too big for u32 the default slippage will be smaller
                // so we can safely use it as a fallback
                .unwrap_or(default_slippage_bps),
            default_slippage_bps,
        );

        Slippage::percentage_from_basis_points(final_slippage_bps)
    }

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
        );

        tracing::debug!("querying 1Inch swap api with {:?}", query);
        let swap = match self.client.get_swap(query).await? {
            RestResponse::Ok(swap) => swap,
            RestResponse::Err(error) if error.description == "insufficient liquidity" => {
                // This means the order cannot get matched which shouldn't be treated as an error.
                return Ok(None);
            }
            RestResponse::Err(error) => return Err((error).into()),
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

impl Interaction for Swap {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.tx.to, self.tx.value, Bytes(self.tx.data.clone()))]
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
        let slippage = Self::compute_max_slippage(
            auction.external_prices.price(&order.buy_token).expect(
                "auction should only contain orders where prices \
                    for buy_token and sell_token are known",
            ),
            &order.buy_amount,
            self.oneinch_slippage_bps,
            &self.max_slippage_in_wei,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::allowances::{Approval, MockAllowanceManaging};
    use crate::liquidity::LimitOrder;
    use crate::solver::ExternalPrices;
    use crate::test::account;
    use contracts::{GPv2Settlement, WETH9};
    use ethcontract::{Web3, H160, U256};
    use mockall::{predicate::*, Sequence};
    use model::order::{Order, OrderData, OrderKind};
    use shared::oneinch_api::{MockOneInchClient, Protocols, Spender};
    use shared::{dummy_contract, transport::create_env_test_transport};


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
            oneinch_slippage_bps: 10u32,
            max_slippage_in_wei: U256::MAX,
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

    #[test]
    fn limits_max_slippage() {
        let slippage = OneInchSolver::compute_max_slippage(
            &U256::exp10(9).to_big_rational(), // USDC price in wei
            &U256::exp10(12),                  // USDC buy amount
            10,                                // default slippage in bps
            &U256::exp10(17),                  // max slippage in wei
        )
        .unwrap();
        assert_eq!(slippage, Slippage::percentage_from_basis_points(1).unwrap());
    }

    #[test]
    fn limits_max_slippage_second() {
        let slippage = OneInchSolver::compute_max_slippage(
            &BigRational::new(2.into(), 1000.into()),  // price in wei
            &U256::exp10(23),                              // buy amount
            10,                                            // default slippage in bps
            &U256::exp10(17),                              // max slippage in wei
        )
        .unwrap();
        assert_eq!(slippage, Slippage::percentage_from_basis_points(5).unwrap());
    }

    #[test]
    fn limits_max_slippage_third() {
        let slippage = OneInchSolver::compute_max_slippage(
            &U256::exp10(9).to_big_rational(),  // USDC price in wei
            &U256::exp10(8),                    // USDC buy amount
            10,                                 // default slippage in bps
            &U256::exp10(17),                   // max slippage in wei
        )
        .unwrap();
        assert_eq!(slippage, Slippage::percentage_from_basis_points(10).unwrap());
    }

    #[test]
    fn limits_max_slippage_fourth() {
        let slippage = OneInchSolver::compute_max_slippage(
            &U256::exp10(9).to_big_rational(), // USDC price in wei
            &U256::exp10(17),                  // USDC buy amount
            10,                                // default slippage in bps
            &U256::exp10(17),                  // max slippage in wei
        )
        .unwrap();
        assert_eq!(slippage, Slippage::percentage_from_basis_points(0).unwrap());
    }

    #[tokio::test]
    async fn test_satisfies_limit_price() {
        let mut client = MockOneInchClient::new();
        let mut allowance_fetcher = MockAllowanceManaging::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let native_token = H160::from_low_u64_be(3);

        client.expect_get_spender().returning(|| {
            Ok(Spender {
                address: H160::zero(),
            })
        });
        client.expect_get_swap().returning(|_| {
            Ok(RestResponse::Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 99.into(),
                ..Default::default()
            }))
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
            Ok(Protocols {
                protocols: vec!["GoodProtocol".into(), "BadProtocol".into()],
            })
        });
        client.expect_get_spender().returning(|| {
            Ok(Spender {
                address: H160::zero(),
            })
        });
        client.expect_get_swap().times(1).returning(|query| {
            assert_eq!(query.quote.protocols, Some(vec!["GoodProtocol".into()]));
            Ok(RestResponse::Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 100.into(),
                ..Default::default()
            }))
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
            .returning(move || Ok(Spender { address: spender }));
        client.expect_get_swap().returning(|_| {
            Ok(RestResponse::Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 100.into(),
                ..Default::default()
            }))
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
            10u32,
            0.into(), // ignored for this test
        )
        .unwrap();
        let slippage = Slippage::percentage_from_basis_points(solver.oneinch_slippage_bps).unwrap();
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
                slippage,
            )
            .await
            .unwrap()
            .unwrap();

        println!("{:#?}", settlement);
    }
}
