//! Solver using the Balancer SOR.

use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    encoding::EncodedInteraction,
    interactions::{
        allowances::{AllowanceManaging, ApprovalRequest},
        balancer_v2::{self, SwapKind},
    },
    liquidity::{slippage::SlippageCalculator, LimitOrder},
    settlement::{Interaction, Settlement},
};
use anyhow::Result;
use contracts::{BalancerV2Vault, GPv2Settlement};
use ethcontract::{Account, Bytes, I256, U256};
use maplit::hashmap;
use model::order::OrderKind;
use shared::balancer_sor_api::{BalancerSorApi, Query, Quote};
use std::sync::Arc;

/// A GPv2 solver that matches GP orders to direct 0x swaps.
pub struct BalancerSorSolver {
    account: Account,
    vault: BalancerV2Vault,
    settlement: GPv2Settlement,
    api: Arc<dyn BalancerSorApi>,
    allowance_fetcher: Arc<dyn AllowanceManaging>,
    slippage_calculator: SlippageCalculator,
}

impl BalancerSorSolver {
    pub fn new(
        account: Account,
        vault: BalancerV2Vault,
        settlement: GPv2Settlement,
        api: Arc<dyn BalancerSorApi>,
        allowance_fetcher: Arc<dyn AllowanceManaging>,
        slippage_calculator: SlippageCalculator,
    ) -> Self {
        Self {
            account,
            vault,
            settlement,
            api,
            allowance_fetcher,
            slippage_calculator,
        }
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for BalancerSorSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<Settlement>, SettlementError> {
        let amount = match order.kind {
            OrderKind::Sell => order.sell_amount,
            OrderKind::Buy => order.buy_amount,
        };
        let query = Query {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            order_kind: order.kind,
            amount,
            gas_price: U256::from_f64_lossy(auction.gas_price),
        };

        let quote = match self.api.quote(query).await? {
            Some(quote) => quote,
            None => {
                tracing::debug!("No route found");
                return Ok(None);
            }
        };

        let (quoted_sell_amount, quoted_buy_amount) = match order.kind {
            OrderKind::Sell => (quote.swap_amount, quote.return_amount),
            OrderKind::Buy => (quote.return_amount, quote.swap_amount),
        };

        if !execution_respects_order(&order, quoted_sell_amount, quoted_buy_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let slippage = self.slippage_calculator.auction_context(auction);
        let (quoted_sell_amount_with_slippage, quoted_buy_amount_with_slippage) = match order.kind {
            OrderKind::Sell => (
                quoted_sell_amount,
                slippage.apply_to_amount_out(order.buy_token, quoted_buy_amount)?,
            ),
            OrderKind::Buy => (
                slippage.apply_to_amount_in(order.sell_token, quoted_sell_amount)?,
                quoted_buy_amount,
            ),
        };

        let prices = hashmap! {
            order.sell_token => quoted_buy_amount,
            order.buy_token => quoted_sell_amount,
        };
        let approval = self
            .allowance_fetcher
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: self.vault.address(),
                amount: quoted_sell_amount_with_slippage,
            })
            .await?;
        let limits = compute_swap_limits(
            &quote,
            quoted_sell_amount_with_slippage,
            quoted_buy_amount_with_slippage,
        )?;
        let batch_swap = BatchSwap {
            vault: self.vault.clone(),
            settlement: self.settlement.clone(),
            kind: order.kind,
            quote,
            limits,
        };

        let mut settlement = Settlement::new(prices);
        settlement.with_liquidity(&order, order.full_execution_amount())?;
        settlement.encoder.append_to_execution_plan(approval);
        settlement.encoder.append_to_execution_plan(batch_swap);

        Ok(Some(settlement))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "BalancerSOR"
    }
}

fn compute_swap_limits(
    quote: &Quote,
    quoted_sell_amount_with_slippage: U256,
    quoted_buy_amount_with_slippage: U256,
) -> Result<Vec<I256>> {
    quote
        .token_addresses
        .iter()
        .map(|&token| -> Result<I256> {
            let limit = if token == quote.token_in {
                // Use positive swap limit for sell amounts (that is, maximum
                // amount that can be transferred in)
                quoted_sell_amount_with_slippage.try_into()?
            } else if token == quote.token_out {
                // Use negative swap limit for buy amounts (that is, minimum
                // amount that must be transferred out)
                I256::try_from(quoted_buy_amount_with_slippage)?
                    .checked_neg()
                    .expect("positive integer can't overflow negation")
            } else {
                // For other tokens we don't want any net transfer in or out.
                I256::zero()
            };

            Ok(limit)
        })
        .collect()
}

#[derive(Debug)]
struct BatchSwap {
    vault: BalancerV2Vault,
    settlement: GPv2Settlement,
    kind: OrderKind,
    quote: Quote,
    limits: Vec<I256>,
}

impl Interaction for BatchSwap {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let kind = match self.kind {
            OrderKind::Sell => SwapKind::GivenIn,
            OrderKind::Buy => SwapKind::GivenOut,
        } as _;
        let swaps = self
            .quote
            .swaps
            .iter()
            .map(|swap| {
                (
                    Bytes(swap.pool_id.0),
                    swap.asset_in_index.into(),
                    swap.asset_out_index.into(),
                    swap.amount,
                    Bytes(swap.user_data.clone()),
                )
            })
            .collect();
        let assets = self.quote.token_addresses.clone();
        let funds = (
            self.settlement.address(), // sender
            false,                     // fromInternalBalance
            self.settlement.address(), // recipient
            false,                     // toInternalBalance
        );
        let limits = self.limits.clone();

        let calldata = self
            .vault
            .methods()
            .batch_swap(kind, swaps, assets, funds, limits, *balancer_v2::NEVER)
            .tx
            .data
            .expect("no calldata")
            .0;

        vec![(self.vault.address(), 0.into(), Bytes(calldata))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::allowances::{AllowanceManager, Approval, MockAllowanceManaging};
    use ethcontract::{H160, H256};
    use mockall::predicate::*;
    use model::order::{Order, OrderData};
    use reqwest::Client;
    use shared::{
        addr,
        balancer_sor_api::{DefaultBalancerSorApi, MockBalancerSorApi, Swap},
        dummy_contract,
        transport::create_env_test_transport,
        Web3,
    };
    use std::env;

    #[test]
    fn computed_swap_sets_sign() {
        let quote = Quote {
            token_in: H160([1; 20]),
            swap_amount: 1000000.into(),
            token_out: H160([3; 20]),
            return_amount: 1000000.into(),
            token_addresses: vec![H160([1; 20]), H160([2; 20]), H160([3; 20])],
            ..Default::default()
        };

        assert_eq!(
            compute_swap_limits(&quote, 1000000.into(), 999000.into()).unwrap(),
            vec![1000000.into(), 0.into(), (-999000).into()],
        );
    }

    #[tokio::test]
    async fn sell_order_swap() {
        let sell_token = addr!("ba100000625a3754423978a60c9317c58a424e3d");
        let buy_token = addr!("6b175474e89094c44da98b954eedeac495271d0f");
        let sell_amount = U256::from(1_000_000);
        let buy_amount = U256::from(2_000_000);

        let vault = dummy_contract!(BalancerV2Vault, H160([0xba; 20]));
        let settlement = dummy_contract!(GPv2Settlement, H160([0x90; 20]));

        let mut api = MockBalancerSorApi::new();
        api.expect_quote()
            .with(eq(Query {
                sell_token,
                buy_token,
                order_kind: OrderKind::Sell,
                amount: sell_amount,
                gas_price: 100_000_000_000_u128.into(),
            }))
            .returning(move |_| {
                Ok(Some(Quote {
                    swap_amount: sell_amount,
                    return_amount: buy_amount,
                    token_in: sell_token,
                    token_out: buy_token,
                    token_addresses: vec![sell_token, H160([0xff; 20]), buy_token],
                    swaps: vec![
                        Swap {
                            pool_id: H256([0; 32]),
                            asset_in_index: 0,
                            asset_out_index: 1,
                            amount: sell_amount,
                            user_data: Default::default(),
                        },
                        Swap {
                            pool_id: H256([1; 32]),
                            asset_in_index: 1,
                            asset_out_index: 2,
                            amount: 0.into(),
                            user_data: Default::default(),
                        },
                    ],
                    ..Default::default()
                }))
            });

        let mut allowance_fetcher = MockAllowanceManaging::new();
        allowance_fetcher
            .expect_get_approval()
            .with(eq(ApprovalRequest {
                token: sell_token,
                spender: vault.address(),
                amount: sell_amount,
            }))
            .returning(|_| Ok(Approval::AllowanceSufficient));

        let solver = BalancerSorSolver::new(
            Account::Local(H160([0x42; 20]), None),
            vault.clone(),
            settlement.clone(),
            Arc::new(api),
            Arc::new(allowance_fetcher),
            SlippageCalculator::default(),
        );

        let result = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token,
                        buy_token,
                        sell_amount,
                        buy_amount,
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 100e9,
                    ..Auction::default()
                },
            )
            .await
            .unwrap()
            .unwrap()
            .encoder
            .finish();

        assert_eq!(result.tokens, [buy_token, sell_token]);
        assert_eq!(result.clearing_prices, [sell_amount, buy_amount]);

        let Bytes(calldata) = &result.interactions[1][0].2;
        assert_eq!(
            calldata,
            &vault
                .methods()
                .batch_swap(
                    SwapKind::GivenIn as _,
                    vec![
                        (
                            Bytes([0; 32]),
                            0.into(),
                            1.into(),
                            sell_amount,
                            Bytes(Default::default())
                        ),
                        (
                            Bytes([1; 32]),
                            1.into(),
                            2.into(),
                            0.into(),
                            Bytes(Default::default())
                        )
                    ],
                    vec![sell_token, H160([0xff; 20]), buy_token],
                    (settlement.address(), false, settlement.address(), false),
                    vec![
                        I256::from_raw(sell_amount),
                        I256::zero(),
                        -I256::from_raw(buy_amount * 999 / 1000)
                    ],
                    U256::one() << 255,
                )
                .tx
                .data
                .unwrap()
                .0,
        );
    }

    #[tokio::test]
    async fn buy_order_swap() {
        let sell_token = addr!("ba100000625a3754423978a60c9317c58a424e3d");
        let buy_token = addr!("6b175474e89094c44da98b954eedeac495271d0f");
        let sell_amount = U256::from(1_000_000);
        let buy_amount = U256::from(2_000_000);

        let vault = dummy_contract!(BalancerV2Vault, H160([0xba; 20]));
        let settlement = dummy_contract!(GPv2Settlement, H160([0x90; 20]));

        let mut api = MockBalancerSorApi::new();
        api.expect_quote()
            .with(eq(Query {
                sell_token,
                buy_token,
                order_kind: OrderKind::Buy,
                amount: buy_amount,
                gas_price: 100_000_000_000_u128.into(),
            }))
            .returning(move |_| {
                Ok(Some(Quote {
                    swap_amount: buy_amount,
                    return_amount: sell_amount,
                    token_in: sell_token,
                    token_out: buy_token,
                    token_addresses: vec![sell_token, buy_token],
                    swaps: vec![Swap {
                        pool_id: Default::default(),
                        asset_in_index: 0,
                        asset_out_index: 1,
                        amount: buy_amount,
                        user_data: Default::default(),
                    }],
                    ..Default::default()
                }))
            });

        let mut allowance_fetcher = MockAllowanceManaging::new();
        allowance_fetcher
            .expect_get_approval()
            .with(eq(ApprovalRequest {
                token: sell_token,
                spender: vault.address(),
                amount: sell_amount * 1001 / 1000,
            }))
            .returning(|_| Ok(Approval::AllowanceSufficient));

        let solver = BalancerSorSolver::new(
            Account::Local(H160([0x42; 20]), None),
            vault.clone(),
            settlement.clone(),
            Arc::new(api),
            Arc::new(allowance_fetcher),
            SlippageCalculator::default(),
        );

        let result = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token,
                        buy_token,
                        sell_amount,
                        buy_amount,
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 100e9,
                    ..Auction::default()
                },
            )
            .await
            .unwrap()
            .unwrap()
            .encoder
            .finish();

        assert_eq!(result.tokens, [buy_token, sell_token]);
        assert_eq!(result.clearing_prices, [sell_amount, buy_amount]);

        let Bytes(calldata) = &result.interactions[1][0].2;
        assert_eq!(
            calldata,
            &vault
                .methods()
                .batch_swap(
                    SwapKind::GivenOut as _,
                    vec![(
                        Bytes([0; 32]),
                        0.into(),
                        1.into(),
                        buy_amount,
                        Bytes(Default::default())
                    )],
                    vec![sell_token, buy_token],
                    (settlement.address(), false, settlement.address(), false),
                    vec![
                        I256::from_raw(sell_amount * 1001 / 1000),
                        -I256::from_raw(buy_amount),
                    ],
                    U256::one() << 255,
                )
                .tx
                .data
                .unwrap()
                .0,
        );
    }

    #[tokio::test]
    async fn skips_settlement_on_empty_swaps() {
        let vault = dummy_contract!(BalancerV2Vault, H160([0xba; 20]));
        let settlement = dummy_contract!(GPv2Settlement, H160([0x90; 20]));

        let mut api = MockBalancerSorApi::new();
        api.expect_quote().returning(move |_| Ok(None));

        let allowance_fetcher = MockAllowanceManaging::new();

        let solver = BalancerSorSolver::new(
            Account::Local(H160([0x42; 20]), None),
            vault,
            settlement,
            Arc::new(api),
            Arc::new(allowance_fetcher),
            SlippageCalculator::default(),
        );

        assert!(matches!(
            solver
                .try_settle_order(LimitOrder::default(), &Auction::default())
                .await,
            Ok(None),
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_sor_solve() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();

        let vault = BalancerV2Vault::deployed(&web3).await.unwrap();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let url = env::var("BALANCER_SOR_URL").unwrap();
        let api = DefaultBalancerSorApi::new(Client::new(), url, chain_id).unwrap();

        let allowance_fetcher = AllowanceManager::new(web3, settlement.address());

        let solver = BalancerSorSolver::new(
            Account::Local(addr!("a6DDBD0dE6B310819b49f680F65871beE85f517e"), None),
            vault,
            settlement,
            Arc::new(api),
            Arc::new(allowance_fetcher),
            SlippageCalculator::default(),
        );

        let sell_settlement = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                        buy_token: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                        sell_amount: 1_000_000_000_000_000_000_u128.into(),
                        buy_amount: 1u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 100e9,
                    ..Auction::default()
                },
            )
            .await
            .unwrap()
            .unwrap();
        println!("Found settlement for sell order: {:#?}", sell_settlement);

        let buy_settlement = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token: addr!("ba100000625a3754423978a60c9317c58a424e3d"),
                        buy_token: addr!("6b175474e89094c44da98b954eedeac495271d0f"),
                        sell_amount: u128::MAX.into(),
                        buy_amount: 100_000_000_000_000_000_000_u128.into(),
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 100e9,
                    ..Auction::default()
                },
            )
            .await
            .unwrap()
            .unwrap();
        println!("Found settlement for buy order: {:#?}", buy_settlement);
    }
}
