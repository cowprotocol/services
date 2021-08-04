//! Module containing implementation of the Matcha solver.
//!
//! This solver will simply use the Matcha API to get a quote for a
//! single GPv2 order and produce a settlement directly against Matcha.
//!
//! Please be aware of the following subtlety for buy orders:
//! 0x's API is adding the defined slippage on the sellAmount of a buy order
//! and then returns the surplus in the buy amount to the user.
//! I.e. if the user defines a 5% slippage, they will sell 5% more, and receive 5%
//! more buy-tokens than ordered. Here is on example tx:
//! https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block=12735030&blockIndex=0&from=0xa6ddbd0de6b310819b49f680f65871bee85f517e&gas=8000000&gasPrice=0&value=0&contractAddress=0x3328f5f2cecaf00a2443082b657cedeaf70bfaef&rawFunctionInput=0x13d79a0b000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000003600000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000029143e200000000000000000000000000000000000000000000000000470de4df820000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000036416d81e590ff67370e4523b9cd3257aa0a853c000000000000000000000000000000000000000000000000000000000291f64800000000000000000000000000000000000000000000000000470de4df8200000000000000000000000000000000000000000000000000000000000060dc5839000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000003dc140000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000029143e2000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000410a7f27a6638cc9cdaba8266a15acef4cf7e1e1c9b9b2059391b7230b67bdfeb21f1d3aa45852f527a5040d3d7a190b92764a2c854f334b7eed579b390b85fd3f1b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000003800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000120000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b3000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25effffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000128d9627aa400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000002b220e100000000000000000000000000000000000000000000000000470de4df82000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2869584cd000000000000000000000000100000000000000000000000000000000000001100000000000000000000000000000000000000000000003239e38b8a60dc53b70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000&network=1
//! This behavior has the following risks: The additional sell tokens from the slippage
//! are not provided by the user, hence the additional tokens might be not available in
//! the settlement contract. For smaller amounts this is unlikely, as we always charge the
//! fees also in the sell token, though, the fee's might not always be sufficient.
//! This risk should be covered in a future PR.
//!
//! Sell orders are unproblematic, especially, since the positive slippage is handed back from matcha

pub mod api;

use super::solver_utils::Slippage;
use crate::interactions::allowances::{AllowanceManager, AllowanceManaging};
use crate::solver::matcha_solver::api::{MatchaApi, MatchaResponseError};
use anyhow::{anyhow, ensure, Result};
use contracts::GPv2Settlement;
use ethcontract::{Account, Bytes};
use maplit::hashmap;
use reqwest::Client;

use super::single_order_solver::SingleOrderSolving;

use self::api::{DefaultMatchaApi, SwapQuery, SwapResponse};
use crate::solver::solver_utils::SettlementError;
use crate::{
    encoding::EncodedInteraction,
    liquidity::LimitOrder,
    settlement::{Interaction, Settlement},
};
use model::order::OrderKind;
use shared::Web3;
use std::fmt::{self, Display, Formatter};

/// Constant maximum slippage of 5 BPS (0.05%) to use for on-chain liquidity.
pub const STANDARD_MATCHA_SLIPPAGE_BPS: u16 = 5;

/// A GPv2 solver that matches GP orders to direct Matcha swaps.
pub struct MatchaSolver {
    account: Account,
    client: Box<dyn MatchaApi + Send + Sync>,
    allowance_fetcher: Box<dyn AllowanceManaging>,
}

/// Chain ID for Mainnet.
const MAINNET_CHAIN_ID: u64 = 1;

impl MatchaSolver {
    pub fn new(
        account: Account,
        web3: Web3,
        settlement_contract: GPv2Settlement,
        chain_id: u64,
        client: Client,
    ) -> Result<Self> {
        ensure!(
            chain_id == MAINNET_CHAIN_ID,
            "Matcha solver only supported on Mainnet",
        );
        let allowance_fetcher = AllowanceManager::new(web3, settlement_contract.address());
        Ok(Self {
            account,
            allowance_fetcher: Box::new(allowance_fetcher),
            client: Box::new(DefaultMatchaApi::new(
                DefaultMatchaApi::DEFAULT_URL,
                client,
            )?),
        })
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for MatchaSolver {
    async fn settle_order(&self, order: LimitOrder) -> Result<Option<Settlement>> {
        let max_retries = 2;
        for _ in 0..max_retries {
            match self.try_settle_order(order.clone()).await {
                Ok(settlement) => return Ok(settlement),
                Err(err) if err.retryable => {
                    tracing::debug!("Retrying Matcha settlement due to: {:?}", &err);
                    continue;
                }
                Err(err) => return Err(err.inner),
            }
        }
        // One last attempt, else throw converted error
        self.try_settle_order(order).await.map_err(|err| err.inner)
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "Matcha"
    }
}

impl From<MatchaResponseError> for SettlementError {
    fn from(err: MatchaResponseError) -> Self {
        SettlementError {
            inner: anyhow!("Matcha Response Error {:?}", err),
            retryable: matches!(err, MatchaResponseError::ServerError(_)),
        }
    }
}

impl MatchaSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
    ) -> Result<Option<Settlement>, SettlementError> {
        let (buy_amount, sell_amount) = match order.kind {
            OrderKind::Buy => (Some(order.buy_amount), None),
            OrderKind::Sell => (None, Some(order.sell_amount)),
        };
        let query = SwapQuery {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount,
            buy_amount,
            slippage_percentage: Slippage::number_from_basis_points(STANDARD_MATCHA_SLIPPAGE_BPS)
                .unwrap(),
            skip_validation: Some(true),
        };

        tracing::debug!("querying Matcha swap api with {:?}", query);
        let swap = self.client.get_swap(query).await?;
        tracing::debug!("proposed Matcha swap is {:?}", swap);

        if !swap_respects_limit_price(&swap, &order) {
            tracing::debug!("Order limit price not respected");
            return Ok(None);
        }

        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => swap.buy_amount,
            order.buy_token => swap.sell_amount,
        });
        let spender = swap.allowance_target;

        settlement.with_liquidity(&order, order.full_execution_amount())?;

        settlement.encoder.append_to_execution_plan(
            self.allowance_fetcher
                .get_approval(order.sell_token, spender, swap.sell_amount)
                .await?,
        );
        settlement.encoder.append_to_execution_plan(swap);
        Ok(Some(settlement))
    }
}

fn swap_respects_limit_price(swap: &SwapResponse, order: &LimitOrder) -> bool {
    match order.kind {
        OrderKind::Buy => swap.sell_amount <= order.sell_amount,
        OrderKind::Sell => swap.buy_amount >= order.buy_amount,
    }
}

impl Interaction for SwapResponse {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.to, self.value, Bytes(self.data.0.clone()))]
    }
}

impl Display for MatchaSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "MatchaSolver")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::allowances::{Approval, MockAllowanceManaging};
    use crate::liquidity::tests::CapturingSettlementHandler;
    use crate::liquidity::LimitOrder;
    use crate::solver::matcha_solver::api::MockMatchaApi;
    use crate::test::account;
    use contracts::{GPv2Settlement, WETH9};
    use ethcontract::{Web3, H160, U256};
    use mockall::predicate::*;
    use mockall::Sequence;
    use model::order::{Order, OrderCreation, OrderKind};
    use shared::transport::{create_env_test_transport, create_test_transport};

    #[tokio::test]
    #[ignore]
    async fn solve_sell_order_on_matcha() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = shared::addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let solver =
            MatchaSolver::new(account(), web3, settlement, chain_id, Client::new()).unwrap();
        let settlement = solver
            .settle_order(
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
            )
            .await
            .unwrap();

        println!("{:#?}", settlement);
    }

    #[tokio::test]
    #[ignore]
    async fn solve_buy_order_on_matcha() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = shared::addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let solver =
            MatchaSolver::new(account(), web3, settlement, chain_id, Client::new()).unwrap();
        let settlement = solver
            .settle_order(
                Order {
                    order_creation: OrderCreation {
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
            )
            .await
            .unwrap();

        println!("{:#?}", settlement);
    }

    #[tokio::test]
    async fn test_satisfies_limit_price_for_orders() {
        let mut client = Box::new(MockMatchaApi::new());
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        let allowance_target = shared::addr!("def1c0ded9bec7f1a1670819833240f027b25eff");
        client.expect_get_swap().returning(move |_| {
            Ok(SwapResponse {
                sell_amount: U256::from_dec_str("100").unwrap(),
                buy_amount: U256::from_dec_str("91").unwrap(),
                allowance_target,
                price: 0.91_f64,
                to: shared::addr!("0000000000000000000000000000000000000000"),
                data: web3::types::Bytes(hex::decode("00").unwrap()),
                value: U256::from_dec_str("0").unwrap(),
            })
        });

        allowance_fetcher
            .expect_get_approval()
            .times(2)
            .with(eq(sell_token), eq(allowance_target), eq(U256::from(100)))
            .returning(move |_, _, _| {
                Ok(Approval::Approve {
                    token: sell_token,
                    spender: allowance_target,
                })
            });

        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
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
            .settle_order(sell_order_passing_limit)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.clearing_prices(),
            &hashmap! {
                sell_token => 91.into(),
                buy_token => 100.into(),
            }
        );

        let result = solver
            .settle_order(sell_order_violating_limit)
            .await
            .unwrap();
        assert!(result.is_none());

        let result = solver
            .settle_order(buy_order_passing_limit)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.clearing_prices(),
            &hashmap! {
                sell_token => 91.into(),
                buy_token => 100.into(),
            }
        );

        let result = solver
            .settle_order(buy_order_violating_limit)
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

        assert!(MatchaSolver::new(account(), web3, settlement, chain_id, Client::new()).is_err())
    }

    #[tokio::test]
    async fn test_sets_allowance_if_necessary() {
        let mut client = Box::new(MockMatchaApi::new());
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        let allowance_target = shared::addr!("def1c0ded9bec7f1a1670819833240f027b25eff");
        client.expect_get_swap().returning(move |_| {
            Ok(SwapResponse {
                sell_amount: U256::from_dec_str("100").unwrap(),
                buy_amount: U256::from_dec_str("91").unwrap(),
                allowance_target,
                price: 13.121_002_575_170_278_f64,
                to: shared::addr!("0000000000000000000000000000000000000000"),
                data: web3::types::Bytes(hex::decode("").unwrap()),
                value: U256::from_dec_str("0").unwrap(),
            })
        });

        // On first invocation no prior allowance, then max allowance set.
        let mut seq = Sequence::new();
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .with(eq(sell_token), eq(allowance_target), eq(U256::from(100)))
            .returning(move |_, _, _| {
                Ok(Approval::Approve {
                    token: sell_token,
                    spender: allowance_target,
                })
            })
            .in_sequence(&mut seq);
        allowance_fetcher
            .expect_get_approval()
            .times(1)
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient))
            .in_sequence(&mut seq);

        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
        };

        let order = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 100.into(),
            buy_amount: 90.into(),
            ..Default::default()
        };

        // On first run we have two main interactions (approve + swap)
        let result = solver.settle_order(order.clone()).await.unwrap().unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 2);

        // On second run we have only have one main interactions (swap)
        let result = solver.settle_order(order).await.unwrap().unwrap();
        assert_eq!(result.encoder.finish().interactions[1].len(), 1)
    }

    #[tokio::test]
    async fn sets_execution_amount_based_on_kind() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let mut client = Box::new(MockMatchaApi::new());
        client.expect_get_swap().returning(move |_| {
            Ok(SwapResponse {
                sell_amount: 1000.into(),
                buy_amount: 5000.into(),
                allowance_target: shared::addr!("0000000000000000000000000000000000000000"),
                price: 0.,
                to: shared::addr!("0000000000000000000000000000000000000000"),
                data: web3::types::Bytes(vec![]),
                value: 0.into(),
            })
        });

        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());
        allowance_fetcher
            .expect_get_approval()
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
        };

        let order = LimitOrder {
            sell_token,
            buy_token,
            sell_amount: 1234.into(),
            buy_amount: 4321.into(),
            ..Default::default()
        };

        // Sell orders are fully executed
        let handler = CapturingSettlementHandler::arc();
        solver
            .settle_order(LimitOrder {
                kind: OrderKind::Sell,
                settlement_handling: handler.clone(),
                ..order.clone()
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(handler.calls(), vec![1234.into()]);

        // Buy orders are fully executed
        let handler = CapturingSettlementHandler::arc();
        solver
            .settle_order(LimitOrder {
                kind: OrderKind::Buy,
                settlement_handling: handler.clone(),
                ..order
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(handler.calls(), vec![4321.into()]);
    }

    #[tokio::test]
    async fn settle_order_retry_until_succeeds() {
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());
        allowance_fetcher
            .expect_get_approval()
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

        let mut client = Box::new(MockMatchaApi::new());
        let mut seq = Sequence::new();
        client
            .expect_get_swap()
            .times(2)
            .returning(|_| {
                // Retryable error
                Err(MatchaResponseError::ServerError(String::new()))
            })
            .in_sequence(&mut seq);
        client
            .expect_get_swap()
            .times(1)
            .returning(|_| {
                Ok(SwapResponse {
                    ..Default::default()
                })
            })
            .in_sequence(&mut seq);

        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
        };

        let order = LimitOrder {
            ..Default::default()
        };
        assert!(solver.settle_order(order).await.is_ok());
    }

    #[tokio::test]
    async fn settle_order_retry_until_exceeds() {
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());
        allowance_fetcher
            .expect_get_approval()
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

        let mut client = Box::new(MockMatchaApi::new());
        let mut seq = Sequence::new();
        client
            .expect_get_swap()
            .times(3)
            .returning(|_| {
                // Retryable error
                Err(MatchaResponseError::ServerError(String::new()))
            })
            .in_sequence(&mut seq);
        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
        };
        let order = LimitOrder {
            ..Default::default()
        };
        assert!(solver.settle_order(order).await.is_err());
    }

    #[tokio::test]
    async fn settle_order_unretryable_error() {
        let mut allowance_fetcher = Box::new(MockAllowanceManaging::new());
        allowance_fetcher
            .expect_get_approval()
            .returning(|_, _, _| Ok(Approval::AllowanceSufficient));

        let mut client = Box::new(MockMatchaApi::new());
        let mut seq = Sequence::new();
        client
            .expect_get_swap()
            .times(1)
            .returning(|_| {
                // Non-Retryable error
                Err(MatchaResponseError::UnknownMatchaError(String::new()))
            })
            .in_sequence(&mut seq);

        let solver = MatchaSolver {
            account: account(),
            client,
            allowance_fetcher,
        };

        let order = LimitOrder {
            ..Default::default()
        };
        assert!(solver.settle_order(order).await.is_err());
    }
}
