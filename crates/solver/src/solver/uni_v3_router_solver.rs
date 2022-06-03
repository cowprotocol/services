use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    encoding::EncodedInteraction,
    interactions::allowances::{AllowanceManager, AllowanceManaging, ApprovalRequest},
    liquidity::LimitOrder,
    settlement::Settlement,
};
use ethcontract::{Account, Bytes};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use shared::{addr, univ3_router_api, Web3};

pub struct UniV3RouterSolver {
    api: univ3_router_api::Api,
    settlement_contract: H160,
    swap_router_02: H160,
    account: Account,
    allowance: AllowanceManager,
}

impl UniV3RouterSolver {
    pub fn new(
        api: univ3_router_api::Api,
        web3: Web3,
        settlement_contract: H160,
        account: Account,
    ) -> Self {
        Self {
            api,
            settlement_contract,
            // https://docs.uniswap.org/protocol/reference/deployments
            swap_router_02: addr!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            account,
            allowance: AllowanceManager::new(web3, settlement_contract),
        }
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for UniV3RouterSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        _: &Auction,
    ) -> Result<Option<Settlement>, SettlementError> {
        let request = univ3_router_api::Request {
            type_: match order.kind {
                OrderKind::Buy => univ3_router_api::Type::Buy,
                OrderKind::Sell => univ3_router_api::Type::Sell,
            },
            token_in: order.sell_token,
            token_out: order.buy_token,
            amount: match order.kind {
                OrderKind::Buy => order.buy_amount,
                OrderKind::Sell => order.sell_amount,
            },
            recipient: self.settlement_contract,
        };
        tracing::debug!(?request);
        let response = self.api.request(&request).await?;
        tracing::debug!(?response);
        let (executed_buy_amount, executed_sell_amount) = match order.kind {
            OrderKind::Buy => (order.buy_amount, response.quote),
            OrderKind::Sell => (response.quote, order.sell_amount),
        };
        if !execution_respects_order(&order, executed_sell_amount, executed_buy_amount) {
            tracing::debug!("does not respect limit price");
            return Ok(None);
        }
        let prices = [
            (order.buy_token, executed_buy_amount),
            (order.sell_token, executed_sell_amount),
        ];
        let mut settlement = Settlement::new(prices.into_iter().collect());
        settlement.with_liquidity(&order, order.full_execution_amount())?;
        let approval = self
            .allowance
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: self.swap_router_02,
                amount: order.sell_amount,
            })
            .await?;
        settlement.encoder.append_to_execution_plan(approval);
        let interaction: EncodedInteraction = (
            self.swap_router_02,
            U256::from(0u32),
            Bytes(response.call_data),
        );
        settlement.encoder.append_to_execution_plan(interaction);
        Ok(Some(settlement))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "UniV3Router"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::WETH9;
    use model::order::{Order, OrderData};

    #[tokio::test]
    #[ignore]
    async fn real() {
        shared::tracing::initialize_for_tests("solver=debug");
        let transport = shared::transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let api = univ3_router_api::Api::new(
            Default::default(),
            "http://localhost:8080".parse().unwrap(),
        );
        let settlement_contract = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let weth = WETH9::deployed(&web3).await.unwrap();
        let solver = UniV3RouterSolver::new(
            api,
            web3,
            settlement_contract.address(),
            crate::test::account(),
        );
        let settlement = solver
            .try_settle_order(
                Order {
                    data: OrderData {
                        sell_token: weth.address(),
                        buy_token: testlib::tokens::DAI,
                        sell_amount: U256::from_f64_lossy(1e18),
                        buy_amount: U256::from(1u32),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                &Auction::default(),
            )
            .await
            .unwrap()
            .unwrap();
        tracing::info!("{:#?}", settlement);
    }
}
