use std::sync::Arc;

use anyhow::{anyhow, Result};
use contracts::{GPv2Settlement, ERC20};
use ethcontract::{Bytes, H160, U256};
use maplit::hashmap;
use shared::{conversions::U256Ext, token_info::TokenInfoFetching};

use super::single_order_solver::SingleOrderSolving;
use crate::{
    encoding::EncodedInteraction,
    interactions::Erc20ApproveInteraction,
    liquidity::LimitOrder,
    settlement::{Interaction, Settlement},
};
use api::{PriceQuery, Side, TransactionBuilderQuery};

use self::api::{DefaultParaswapApi, ParaswapApi, PriceResponse, TransactionBuilderResponse};

mod api;

const REFERRER: &str = "GPv2";
const APPROVAL_RECEIVER: H160 = shared::addr!("b70bc06d2c9bf03b3373799606dc7d39346c06b3");

/// A GPv2 solver that matches GP orders to direct ParaSwap swaps.
pub struct ParaswapSolver<F> {
    settlement_contract: GPv2Settlement,
    solver_address: H160,
    token_info: Arc<dyn TokenInfoFetching>,
    allowance_fetcher: F,
    client: Box<dyn ParaswapApi + Send + Sync>,
}

impl ParaswapSolver<GPv2Settlement> {
    pub fn new(
        settlement_contract: GPv2Settlement,
        solver_address: H160,
        token_info: Arc<dyn TokenInfoFetching>,
    ) -> Self {
        let allowance_fetcher = settlement_contract.clone();
        Self {
            settlement_contract,
            solver_address,
            token_info,
            allowance_fetcher,
            client: Box::new(DefaultParaswapApi::default()),
        }
    }
}

impl<F> std::fmt::Debug for ParaswapSolver<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ParaswapSolver")
    }
}

/// Helper trait to mock the smart contract interaction
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait AllowanceFetching: Send + Sync {
    async fn existing_allowance(&self, token: H160, spender: H160) -> Result<U256>;
}

#[async_trait::async_trait]
impl AllowanceFetching for GPv2Settlement {
    async fn existing_allowance(&self, token: H160, spender: H160) -> Result<U256> {
        let token_contract = ERC20::at(&self.raw_instance().web3(), token);
        Ok(token_contract
            .allowance(self.address(), spender)
            .call()
            .await?)
    }
}

#[async_trait::async_trait]
impl<F> SingleOrderSolving for ParaswapSolver<F>
where
    F: AllowanceFetching,
{
    async fn settle_order(&self, order: LimitOrder) -> Result<Option<Settlement>> {
        let (amount, side) = match order.kind {
            model::order::OrderKind::Buy => (order.buy_amount, Side::Buy),
            model::order::OrderKind::Sell => (order.sell_amount, Side::Sell),
        };
        let token_infos = self
            .token_info
            .get_token_infos(&[order.sell_token, order.buy_token])
            .await;
        let decimals = |token: &H160| {
            token_infos
                .get(token)
                .and_then(|info| info.decimals.map(usize::from))
                .ok_or_else(|| anyhow!("decimals for token {:?} not found", token))
        };

        let price_query = PriceQuery {
            from: order.sell_token,
            to: order.buy_token,
            from_decimals: decimals(&order.sell_token)?,
            to_decimals: decimals(&order.buy_token)?,
            amount,
            side,
        };

        tracing::debug!("querying Paraswap API with {:?}", price_query);
        let price_response = self.client.price(price_query).await?;
        if !satisfies_limit_price(&order, &price_response) {
            tracing::debug!("Order limit price not respected");
            return Ok(None);
        }

        // 0.1% slippage
        let dest_amount_with_slippage = price_response
            .dest_amount
            .checked_mul(999.into())
            .ok_or_else(|| anyhow!("Overflow during slippage computation"))?
            / 1000;
        let transaction_query = TransactionBuilderQuery {
            src_token: order.sell_token,
            dest_token: order.buy_token,
            src_amount: price_response.src_amount,
            dest_amount: dest_amount_with_slippage,
            from_decimals: decimals(&order.sell_token)?,
            to_decimals: decimals(&order.buy_token)?,
            price_route: price_response.price_route_raw,
            user_address: self.solver_address,
            referrer: REFERRER.to_string(),
        };
        let transaction = self.client.transaction(transaction_query).await?;

        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => price_response.dest_amount,
            order.buy_token => price_response.src_amount,
        });
        settlement.with_liquidity(&order, amount)?;

        if self
            .allowance_fetcher
            .existing_allowance(order.sell_token, APPROVAL_RECEIVER)
            .await?
            < price_response.src_amount
        {
            let sell_token_contract = ERC20::at(
                &self.settlement_contract.raw_instance().web3(),
                order.sell_token,
            );
            settlement
                .encoder
                .append_to_execution_plan(Erc20ApproveInteraction {
                    token: sell_token_contract,
                    spender: APPROVAL_RECEIVER,
                    amount: U256::MAX,
                });
        }
        settlement.encoder.append_to_execution_plan(transaction);
        Ok(Some(settlement))
    }

    fn name(&self) -> &'static str {
        "ParaSwap"
    }
}

impl Interaction for TransactionBuilderResponse {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.to, self.value, Bytes(self.data.0.clone()))]
    }
}

fn satisfies_limit_price(order: &LimitOrder, response: &PriceResponse) -> bool {
    // We check if order.sell / order.buy >= response.sell / response.buy
    order.sell_amount.to_big_rational() * response.dest_amount.to_big_rational()
        >= response.src_amount.to_big_rational() * order.buy_amount.to_big_rational()
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::api::MockParaswapApi;
    use super::*;
    use crate::testutil;
    use mockall::Sequence;
    use shared::token_info::{MockTokenInfoFetching, TokenInfo};

    #[test]
    fn test_satisfies_limit_price() {
        assert_eq!(
            satisfies_limit_price(
                &LimitOrder {
                    sell_amount: 100.into(),
                    buy_amount: 95.into(),
                    ..Default::default()
                },
                &PriceResponse {
                    src_amount: 100.into(),
                    dest_amount: 90.into(),
                    ..Default::default()
                }
            ),
            false
        );

        assert_eq!(
            satisfies_limit_price(
                &LimitOrder {
                    sell_amount: 100.into(),
                    buy_amount: 95.into(),
                    ..Default::default()
                },
                &PriceResponse {
                    src_amount: 100.into(),
                    dest_amount: 100.into(),
                    ..Default::default()
                }
            ),
            true
        );

        assert_eq!(
            satisfies_limit_price(
                &LimitOrder {
                    sell_amount: 100.into(),
                    buy_amount: 95.into(),
                    ..Default::default()
                },
                &PriceResponse {
                    src_amount: 100.into(),
                    dest_amount: 95.into(),
                    ..Default::default()
                }
            ),
            true
        );
    }

    #[tokio::test]
    async fn test_skips_order_if_unable_to_fetch_decimals() {
        let client = Box::new(MockParaswapApi::new());
        let allowance_fetcher = MockAllowanceFetching::new();
        let mut token_info = MockTokenInfoFetching::new();

        token_info
            .expect_get_token_infos()
            .return_const(HashMap::new());

        let solver = ParaswapSolver {
            client,
            solver_address: Default::default(),
            token_info: Arc::new(token_info),
            allowance_fetcher,
            settlement_contract: GPv2Settlement::at(&testutil::dummy_web3(), H160::zero()),
        };

        let order = LimitOrder::default();
        let result = solver.settle_order(order).await;

        // This implicitly checks that we don't call the API is its mock doesn't have any expectations and would panic
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_respects_limit_price() {
        let mut client = Box::new(MockParaswapApi::new());
        let mut allowance_fetcher = MockAllowanceFetching::new();
        let mut token_info = MockTokenInfoFetching::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        client.expect_price().returning(|_| {
            Ok(PriceResponse {
                price_route_raw: Default::default(),
                src_amount: 100.into(),
                dest_amount: 99.into(),
            })
        });
        client
            .expect_transaction()
            .returning(|_| Ok(Default::default()));

        allowance_fetcher
            .expect_existing_allowance()
            .returning(|_, _| Ok(U256::zero()));

        token_info.expect_get_token_infos().returning(move |_| {
            hashmap! {
                sell_token => TokenInfo { decimals: Some(18)},
                buy_token => TokenInfo { decimals: Some(18)},
            }
        });

        let solver = ParaswapSolver {
            client,
            solver_address: Default::default(),
            token_info: Arc::new(token_info),
            allowance_fetcher,
            settlement_contract: GPv2Settlement::at(&testutil::dummy_web3(), H160::zero()),
        };

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
            .settle_order(order_passing_limit)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.clearing_prices(),
            &hashmap! {
                sell_token => 99.into(),
                buy_token => 100.into(),
            }
        );

        let result = solver.settle_order(order_violating_limit).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_sets_allowance_if_necessary() {
        let mut client = Box::new(MockParaswapApi::new());
        let mut allowance_fetcher = MockAllowanceFetching::new();
        let mut token_info = MockTokenInfoFetching::new();

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(1);

        client.expect_price().returning(|_| {
            Ok(PriceResponse {
                price_route_raw: Default::default(),
                src_amount: 100.into(),
                dest_amount: 99.into(),
            })
        });
        client
            .expect_transaction()
            .returning(|_| Ok(Default::default()));

        // On first invocation no prior allowance, then max allowance set.
        let mut seq = Sequence::new();
        allowance_fetcher
            .expect_existing_allowance()
            .times(1)
            .returning(|_, _| Ok(U256::zero()))
            .in_sequence(&mut seq);
        allowance_fetcher
            .expect_existing_allowance()
            .times(1)
            .returning(|_, _| Ok(U256::max_value()))
            .in_sequence(&mut seq);

        token_info.expect_get_token_infos().returning(move |_| {
            hashmap! {
                sell_token => TokenInfo { decimals: Some(18)},
                buy_token => TokenInfo { decimals: Some(18)},
            }
        });

        let solver = ParaswapSolver {
            client,
            solver_address: Default::default(),
            token_info: Arc::new(token_info),
            allowance_fetcher,
            settlement_contract: GPv2Settlement::at(&testutil::dummy_web3(), H160::zero()),
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
}
