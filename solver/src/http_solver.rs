mod model;
mod settlement;

use self::model::*;
use crate::{
    liquidity::{AmmOrder, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
use ::model::order::OrderKind;
use anyhow::{ensure, Context, Result};
use primitive_types::H160;
use reqwest::{header::HeaderValue, Client, Url};
use std::collections::{HashMap, HashSet};

// TODO: limit trading for tokens that don't have uniswap - fee pool
// TODO: exclude partially fillable orders
// TODO: find correct ordering for uniswap trades
// TODO: special rounding for the prices we get from the solver?
// TODO: make sure to give the solver disconnected token islands individually

/// The configuration passed as url parameters to the solver.
#[derive(Debug, Default)]
pub struct SolverConfig {
    max_nr_exec_orders: u32,
    time_limit: u32,
    // TODO: add more parameters that we want to set
}

impl SolverConfig {
    fn add_to_query(&self, url: &mut Url) {
        url.query_pairs_mut()
            .append_pair(
                "max_nr_exec_orders",
                self.max_nr_exec_orders.to_string().as_str(),
            )
            .append_pair("time_limit", self.time_limit.to_string().as_str());
    }
}

pub struct HttpSolver {
    base: Url,
    client: Client,
    api_key: Option<String>,
    config: SolverConfig,
}

impl HttpSolver {
    pub fn new(base: Url, api_key: Option<String>, config: SolverConfig) -> Self {
        // Unwrap because we cannot handle client creation failing.
        let client = Client::builder().build().unwrap();
        Self {
            base,
            client,
            api_key,
            config,
        }
    }

    // Solver api requires specifying token as strings. We use the address as a string for now.
    // Later we could use a more meaningful name like the token symbol but we have to ensure
    // uniqueness.
    fn token_to_string(&self, token: &H160) -> String {
        // Token names must start with a letter.
        format!("t{:x}", token)
    }

    // Maps string based token index from solver api
    fn tokens(&self, orders: &[Liquidity]) -> HashMap<String, H160> {
        orders
            .iter()
            .flat_map(|liquidity| match liquidity {
                Liquidity::Limit(order) => {
                    std::iter::once(order.sell_token).chain(std::iter::once(order.buy_token))
                }
                Liquidity::Amm(amm) => {
                    std::iter::once(amm.tokens.get().0).chain(std::iter::once(amm.tokens.get().1))
                }
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .map(|token| (self.token_to_string(&token), token))
            .collect()
    }

    // Maps string based token index from solver api
    fn token_models(&self, tokens: &HashMap<String, H160>) -> HashMap<String, TokenInfoModel> {
        // TODO: gather real decimals and store them in a cache
        tokens
            .iter()
            .map(|(index, _)| (index.clone(), TokenInfoModel { decimals: 18 }))
            .collect()
    }

    // Maps string based order index from solver api
    fn orders<'a>(&self, orders: &'a [Liquidity]) -> HashMap<String, &'a LimitOrder> {
        orders
            .iter()
            .filter_map(|liquidity| match liquidity {
                Liquidity::Limit(order) => Some(order),
                Liquidity::Amm(_) => None,
            })
            .enumerate()
            .map(|(index, order)| (index.to_string(), order))
            .collect()
    }

    // Maps string based order index from solver api
    fn order_models<'a>(
        &self,
        orders: &HashMap<String, &'a LimitOrder>,
    ) -> HashMap<String, OrderModel> {
        orders
            .iter()
            .map(|(index, order)| {
                let order = OrderModel {
                    sell_token: self.token_to_string(&order.sell_token),
                    buy_token: self.token_to_string(&order.buy_token),
                    sell_amount: order.sell_amount,
                    buy_amount: order.buy_amount,
                    allow_partial_fill: order.partially_fillable,
                    is_sell_order: matches!(order.kind, OrderKind::Sell),
                };
                (index.clone(), order)
            })
            .collect()
    }

    // Maps string based amm index from solver api
    fn amms<'a>(&self, orders: &'a [Liquidity]) -> HashMap<String, &'a AmmOrder> {
        orders
            .iter()
            .filter_map(|liquidity| match liquidity {
                Liquidity::Limit(_) => None,
                Liquidity::Amm(amm) => Some(amm),
            })
            .enumerate()
            .map(|(index, amm)| (index.to_string(), amm))
            .collect()
    }

    // Maps string based amm index from solver api
    fn amm_models<'a>(
        &self,
        amms: &HashMap<String, &'a AmmOrder>,
    ) -> HashMap<String, UniswapModel> {
        amms.iter()
            .map(|(index, amm)| {
                let uniswap = UniswapModel {
                    token1: self.token_to_string(&amm.tokens.get().0),
                    token2: self.token_to_string(&amm.tokens.get().1),
                    balance1: amm.reserves.0,
                    balance2: amm.reserves.1,
                    // TODO use AMM fee
                    fee: 0.003,
                    mandatory: false,
                };
                (index.clone(), uniswap)
            })
            .collect()
    }

    async fn send(&self, model: &BatchAuctionModel) -> Result<SettledBatchAuctionModel> {
        let mut url = self.base.clone();
        url.set_path("/solve");
        self.config.add_to_query(&mut url);
        let query = url.query().map(ToString::to_string).unwrap_or_default();
        let mut request = self.client.post(url);
        if let Some(api_key) = &self.api_key {
            let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
            header.set_sensitive(true);
            request = request.header("X-API-KEY", header);
        }
        let body = serde_json::to_string(&model).context("failed to encode body")?;
        let request = request.body(body.clone());
        let response = request.send().await.context("failed to send request")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to decode response body")?;
        let context = || {
            format!(
                "request query {}, request body {}, response body {}",
                query, body, text
            )
        };
        ensure!(
            status.is_success(),
            "solver response is not success: status {}, {}",
            status,
            context()
        );
        serde_json::from_str(text.as_str())
            .with_context(|| format!("failed to decode response json, {}", context()))
    }
}

#[async_trait::async_trait]
impl Solver for HttpSolver {
    async fn solve(&self, liquidity: Vec<Liquidity>) -> Result<Option<Settlement>> {
        let tokens = self.tokens(liquidity.as_slice());
        let orders = self.orders(liquidity.as_slice());
        let amms = self.amms(liquidity.as_slice());
        let ref_token = match tokens.keys().next() {
            Some(token) => token.clone(),
            None => return Ok(None),
        };
        let model = BatchAuctionModel {
            tokens: self.token_models(&tokens),
            orders: self.order_models(&orders),
            uniswaps: self.amm_models(&amms),
            ref_token,
            default_fee: 0.0,
        };
        let settled = self.send(&model).await?;
        settlement::convert_settlement(&settled, &tokens, &orders, &amms).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{
        AmmOrder, LimitOrder, MockAmmSettlementHandling, MockLimitOrderSettlementHandling,
    };
    use ::model::TokenPair;
    use num::Rational;
    use std::sync::Arc;

    // cargo test real_solver -- --ignored --nocapture
    // set the env variable GP_V2_OPTIMIZER_URL to use a non localhost optimizer
    #[tokio::test]
    #[ignore]
    async fn real_solver() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter("solver=trace")
            .init();
        let url = std::env::var("GP_V2_OPTIMIZER_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let solver = HttpSolver::new(
            url.parse().unwrap(),
            None,
            SolverConfig {
                max_nr_exec_orders: 100,
                time_limit: 100,
            },
        );
        let orders = vec![
            Liquidity::Limit(LimitOrder {
                buy_token: H160::zero(),
                sell_token: H160::from_low_u64_be(1),
                buy_amount: 1.into(),
                sell_amount: 2.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: Arc::new(MockLimitOrderSettlementHandling::new()),
            }),
            Liquidity::Amm(AmmOrder {
                tokens: TokenPair::new(H160::zero(), H160::from_low_u64_be(1)).unwrap(),
                reserves: (100, 100),
                fee: Rational::new(1, 1),
                settlement_handling: Arc::new(MockAmmSettlementHandling::new()),
            }),
        ];
        let settlement = solver.solve(orders).await.unwrap().unwrap();
        dbg!(settlement);
    }
}
