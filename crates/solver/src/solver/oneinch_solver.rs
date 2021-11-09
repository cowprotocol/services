//! Module containing implementation of the 1Inch solver.
//!
//! This simple solver will simply use the 1Inch API to get a quote for a
//! single GPv2 order and produce a settlement directly against 1Inch.

pub mod api;
use self::api::{Amount, OneInchClient, Swap, SwapQuery};
use super::single_order_solver::{SettlementError, SingleOrderSolving};
use crate::solver::oneinch_solver::api::SwapResponse;
use crate::{
    encoding::EncodedInteraction,
    interactions::allowances::{AllowanceManager, AllowanceManaging},
    liquidity::{slippage::MAX_SLIPPAGE_BPS, LimitOrder},
    settlement::{Interaction, Settlement},
    solver::oneinch_solver::api::OneInchClientImpl,
};
use anyhow::{ensure, Result};
use contracts::GPv2Settlement;
use derivative::Derivative;
use ethcontract::{Account, Bytes};
use maplit::hashmap;
use model::order::OrderKind;
use reqwest::Client;
use shared::solver_utils::Slippage;
use shared::Web3;
use std::{
    collections::HashSet,
    fmt::{self, Display, Formatter},
};

/// A GPv2 solver that matches GP **sell** orders to direct 1Inch swaps.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct OneInchSolver {
    account: Account,
    settlement_contract: GPv2Settlement,
    disabled_protocols: HashSet<String>,
    #[derivative(Debug = "ignore")]
    client: Box<dyn OneInchClient>,
    #[derivative(Debug = "ignore")]
    allowance_fetcher: Box<dyn AllowanceManaging>,
}

/// Chain ID for Mainnet.
const MAINNET_CHAIN_ID: u64 = 1;

impl OneInchSolver {
    /// Creates a new 1Inch solver with a list of disabled protocols.
    pub fn with_disabled_protocols(
        account: Account,
        web3: Web3,
        settlement_contract: GPv2Settlement,
        chain_id: u64,
        disabled_protocols: impl IntoIterator<Item = String>,
        client: Client,
    ) -> Result<Self> {
        ensure!(
            chain_id == MAINNET_CHAIN_ID,
            "1Inch solver only supported on Mainnet",
        );

        let settlement_address = settlement_contract.address();
        Ok(Self {
            account,
            settlement_contract,
            disabled_protocols: disabled_protocols.into_iter().collect(),
            client: Box::new(OneInchClientImpl::new(
                OneInchClientImpl::DEFAULT_URL,
                client,
            )?),
            allowance_fetcher: Box::new(AllowanceManager::new(web3, settlement_address)),
        })
    }
}

impl OneInchSolver {
    /// Gets the list of supported protocols for the 1Inch solver.
    async fn supported_protocols(&self) -> Result<Option<Vec<String>>> {
        let protocols = if self.disabled_protocols.is_empty() {
            None
        } else {
            Some(
                self.client
                    .get_protocols()
                    .await?
                    .protocols
                    .into_iter()
                    .filter(|protocol| !self.disabled_protocols.contains(protocol))
                    .collect(),
            )
        };
        Ok(protocols)
    }

    /// Settles a single sell order against a 1Inch swap using the specified protocols.
    async fn settle_order_with_protocols(
        &self,
        order: LimitOrder,
        protocols: Option<Vec<String>>,
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
            .get_approval(order.sell_token, spender.address, order.sell_amount)
            .await?;

        let query = SwapQuery {
            from_token_address: order.sell_token,
            to_token_address: order.buy_token,
            amount: order.sell_amount,
            from_address: self.settlement_contract.address(),
            slippage: Slippage::percentage_from_basis_points(MAX_SLIPPAGE_BPS).unwrap(),
            protocols,
            // Disable balance/allowance checks, as the settlement contract
            // does not hold balances to traded tokens.
            disable_estimate: Some(true),
            // Use at most 2 connector tokens
            complexity_level: Some(Amount::new(2).unwrap()),
            // Cap swap gas to 750K.
            gas_limit: Some(750_000),
            // Use only 3 main route for cheaper trades.
            main_route_parts: Some(Amount::new(3).unwrap()),
            parts: Some(Amount::new(3).unwrap()),
        };

        tracing::debug!("querying 1Inch swap api with {:?}", query);
        let swap = match self.client.get_swap(query).await? {
            SwapResponse::Swap(swap) => swap,
            SwapResponse::Error(error) => return Err(error.into()),
        };

        if !satisfies_limit_price(&swap, &order) {
            tracing::debug!("Order limit price not respected");
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

fn satisfies_limit_price(swap: &Swap, order: &LimitOrder) -> bool {
    swap.to_token_amount >= order.buy_amount
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
    ) -> Result<Option<Settlement>, SettlementError> {
        if order.kind != OrderKind::Sell {
            // 1Inch only supports sell orders
            return Ok(None);
        }
        let protocols = self.supported_protocols().await?;
        self.settle_order_with_protocols(order, protocols).await
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
    use super::{api::MockOneInchClient, *};
    use crate::interactions::allowances::{Approval, MockAllowanceManaging};
    use crate::liquidity::LimitOrder;
    use crate::solver::oneinch_solver::api::Protocols;
    use crate::solver::oneinch_solver::api::Spender;
    use crate::test::account;
    use contracts::{GPv2Settlement, WETH9};
    use ethcontract::{Web3, H160, U256};
    use maplit::hashset;
    use mockall::{predicate::*, Sequence};
    use model::order::{Order, OrderCreation, OrderKind};
    use shared::{
        dummy_contract,
        transport::{create_env_test_transport, create_test_transport},
    };
    use std::iter;

    fn dummy_solver(
        client: MockOneInchClient,
        allowance_fetcher: MockAllowanceManaging,
    ) -> OneInchSolver {
        let settlement_contract = dummy_contract!(GPv2Settlement, H160::zero());
        OneInchSolver {
            account: account(),
            settlement_contract,
            disabled_protocols: HashSet::new(),
            client: Box::new(client),
            allowance_fetcher: Box::new(allowance_fetcher),
        }
    }

    #[tokio::test]
    async fn ignores_buy_orders() {
        assert!(
            dummy_solver(MockOneInchClient::new(), MockAllowanceManaging::new())
                .try_settle_order(LimitOrder {
                    kind: OrderKind::Buy,
                    ..Default::default()
                },)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn returns_none_when_no_protocols_are_disabled() {
        let protocols = dummy_solver(MockOneInchClient::new(), MockAllowanceManaging::new())
            .supported_protocols()
            .await
            .unwrap();
        assert!(protocols.is_none());
    }

    #[tokio::test]
    async fn test_satisfies_limit_price() {
        let mut client = MockOneInchClient::new();
        let mut allowance_fetcher = MockAllowanceManaging::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        client.expect_get_spender().returning(|| {
            Ok(Spender {
                address: H160::zero(),
            })
        });
        client.expect_get_swap().returning(|_| {
            Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 99.into(),
                ..Default::default()
            }
            .into())
        });

        allowance_fetcher
            .expect_get_approval()
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

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

        let result = solver
            .try_settle_order(order_passing_limit)
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
            .try_settle_order(order_violating_limit)
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
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

        client.expect_get_protocols().returning(|| {
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
            assert_eq!(query.protocols, Some(vec!["GoodProtocol".into()]));
            Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 100.into(),
                ..Default::default()
            }
            .into())
        });

        let solver = OneInchSolver {
            disabled_protocols: hashset!["BadProtocol".to_string(), "VeryBadProtocol".to_string()],
            ..dummy_solver(client, allowance_fetcher)
        };

        // Limit price violated. Actual assert is happening in `expect_get_swap()`
        assert!(solver
            .try_settle_order(LimitOrder {
                kind: OrderKind::Sell,
                buy_amount: U256::max_value(),
                ..Default::default()
            })
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
            Ok(Swap {
                from_token_amount: 100.into(),
                to_token_amount: 100.into(),
                ..Default::default()
            }
            .into())
        });

        // On first invocation no prior allowance, then max allowance set.
        let mut seq = Sequence::new();
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .with(eq(sell_token), eq(spender), eq(U256::from(100)))
            .returning(move |_, _, _| {
                Ok(Approval::Approve {
                    token: sell_token,
                    spender,
                })
            })
            .in_sequence(&mut seq);
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .with(eq(sell_token), eq(spender), eq(U256::from(100)))
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient))
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

        // On first run we have two main interactions (approve + swap)
        let result = solver
            .try_settle_order(order.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 2);

        // On second run we have only have one main interactions (swap)
        let result = solver.try_settle_order(order).await.unwrap().unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 1)
    }

    #[test]
    fn returns_error_on_non_mainnet() {
        let web3 = Web3::new(create_test_transport("http://never.used"));
        let chain_id = 42;
        let settlement = dummy_contract!(GPv2Settlement, H160::zero());

        assert!(OneInchSolver::with_disabled_protocols(
            account(),
            web3,
            settlement,
            chain_id,
            iter::empty(),
            Client::new(),
        )
        .is_err())
    }

    #[tokio::test]
    #[ignore]
    async fn solve_order_on_oneinch() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = shared::addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let solver = OneInchSolver::with_disabled_protocols(
            account(),
            web3,
            settlement,
            chain_id,
            vec!["PMM1".to_string()],
            Client::new(),
        )
        .unwrap();
        let settlement = solver
            .settle_order_with_protocols(
                Order {
                    order_creation: OrderCreation {
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
            )
            .await
            .unwrap()
            .unwrap();

        println!("{:#?}", settlement);
    }
}
