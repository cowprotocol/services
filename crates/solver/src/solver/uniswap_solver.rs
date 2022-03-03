//! Single order solver based on the Uniswap API.

use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    encoding::EncodedInteraction,
    interactions::allowances::AllowanceManaging,
    liquidity::slippage::{amount_minus_max_slippage, amount_plus_max_slippage},
    settlement::Interaction,
};
use crate::{liquidity::LimitOrder, settlement::Settlement};
use anyhow::Result;
use contracts::UniswapSwapRouter02;
use ethcontract::{Account, Bytes, H160, U256};
use maplit::hashmap;
use model::order::OrderKind;
use shared::{
    addr,
    uniswap_api::{Hop, Protocol, Quote, QuoteQuery, UniswapApi},
};
use std::sync::Arc;

/// A GPv2 solver that matches GP orders to Uniswap V2/V3 routes.
pub struct UniswapSolver {
    chain_id: u64,
    router: UniswapSwapRouter02,
    account: Account,
    api: Arc<dyn UniswapApi>,
    allowances: Arc<dyn AllowanceManaging>,
}

impl UniswapSolver {
    #[allow(dead_code)]
    pub fn new(
        chain_id: u64,
        router: UniswapSwapRouter02,
        account: Account,
        api: Arc<dyn UniswapApi>,
        allowances: Arc<dyn AllowanceManaging>,
    ) -> Self {
        Self {
            chain_id,
            router,
            account,
            api,
            allowances,
        }
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for UniswapSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        _: &Auction,
    ) -> Result<Option<Settlement>, SettlementError> {
        let query = QuoteQuery {
            protocols: Protocol::all(),
            token_in_address: order.sell_token,
            token_in_chain_id: self.chain_id,
            token_out_address: order.buy_token,
            token_out_chain_id: self.chain_id,
            amount: match order.kind {
                OrderKind::Buy => order.buy_amount,
                OrderKind::Sell => order.sell_amount,
            },
            kind: order.kind.into(),
        };
        let quote = self.api.get_quote(&query).await?;

        let is_single_hop = quote.route.len() == 1 && quote.route[0].len() == 1;
        if !is_single_hop {
            // TODO(nlordell): only single hop supported ATM.
            return Ok(None);
        }

        let (quote_sell_amount, quote_buy_amount) = match order.kind {
            OrderKind::Buy => (quote.quote, quote.amount),
            OrderKind::Sell => (quote.amount, quote.quote),
        };
        if !execution_respects_order(&order, quote_sell_amount, quote_buy_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => quote_buy_amount,
            order.buy_token => quote_sell_amount,
        });
        let spender = addr!("0000000000000000000000000000000000000000");

        settlement.with_liquidity(&order, order.full_execution_amount())?;

        let quote_sell_amount_with_slippage = amount_minus_max_slippage(quote_sell_amount);
        settlement.encoder.append_to_execution_plan(
            self.allowances
                .get_approval(order.sell_token, spender, quote_sell_amount_with_slippage)
                .await?,
        );

        settlement
            .encoder
            .append_to_execution_plan((self.router.clone(), quote, order.kind));
        Ok(Some(settlement))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "Uniswap"
    }
}

impl Interaction for (UniswapSwapRouter02, Quote, OrderKind) {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let (router, quote, kind) = self;
        debug_assert!(quote.route.len() == 1 && quote.route[0].len() == 1);
        vec![(
            router.address(),
            U256::zero(),
            Bytes(
                router
                    .multicall(vec![single_hop(router, &quote.route[0][0], *kind)])
                    .tx
                    .data
                    .expect("missing calldata")
                    .0,
            ),
        )]
    }
}

/// Marker address to indicate recipient should the caller of the swap.
const MSG_SENDER: H160 = addr!("0000000000000000000000000000000000000001");

fn single_hop(router: &UniswapSwapRouter02, hop: &Hop, kind: OrderKind) -> Bytes<Vec<u8>> {
    let (amount_exact, amount_limit) = match kind {
        OrderKind::Sell => (hop.amount_in, amount_minus_max_slippage(hop.amount_out)),
        OrderKind::Buy => (hop.amount_out, amount_plus_max_slippage(hop.amount_in)),
    };
    let params = (
        hop.token_in.address,
        hop.token_out.address,
        hop.fee,
        MSG_SENDER, // recipient
        amount_exact,
        amount_limit,
        U256::zero(), // sqrtPriceLimitX96 - no marginal price limit
    );

    let method = match kind {
        OrderKind::Sell => router.exact_input_single(params),
        OrderKind::Buy => router.exact_output_single(params),
    };
    Bytes(method.tx.data.expect("missing calldata").0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::allowances::AllowanceManager;
    use contracts::GPv2Settlement;
    use ethcontract::Web3;
    use model::order::{Order, OrderCreation};
    use shared::{transport::create_env_test_transport, uniswap_api::UniswapHttpApi};

    #[tokio::test]
    #[ignore]
    async fn uniswap_api_solve() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let router = UniswapSwapRouter02::deployed(&web3).await.unwrap();

        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();
        let allowances = AllowanceManager::new(web3.clone(), settlement.address());
        let solver = UniswapSolver::new(
            chain_id,
            router,
            Account::Local(addr!("a6DDBD0dE6B310819b49f680F65871beE85f517e"), None),
            Arc::new(UniswapHttpApi::new(reqwest::Client::new())),
            Arc::new(allowances),
        );

        let sell_settlement = solver
            .try_settle_order(
                Order {
                    creation: OrderCreation {
                        sell_token: addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                        buy_token: addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                        sell_amount: 1_000_000_000_000_000_000_u128.into(),
                        buy_amount: 1u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 50e9,
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
                    creation: OrderCreation {
                        sell_token: addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                        buy_token: addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                        sell_amount: u128::MAX.into(),
                        buy_amount: 3_000_000_000_u128.into(),
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction {
                    gas_price: 50e9,
                    ..Auction::default()
                },
            )
            .await
            .unwrap()
            .unwrap();
        println!("Found settlement for buy order: {:#?}", buy_settlement);
    }
}
