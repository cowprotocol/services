mod model;
mod settlement;

use self::{model::*, settlement::SettlementContext};
use crate::{
    liquidity::{AmmOrder, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
use ::model::order::OrderKind;
use anyhow::{ensure, Context, Result};
use futures::join;
use num::{BigRational, ToPrimitive};
use primitive_types::H160;
use reqwest::{header::HeaderValue, Client, Url};
use shared::{
    price_estimate::{PriceEstimating, PriceEstimationError},
    token_info::{TokenInfo, TokenInfoFetching},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

// Estimates from multivariate linear regression here:
// https://docs.google.com/spreadsheets/d/13UeUQ9DA4bHlcy9-i8d4nSLlCxSfjcXpTelvXYzyJzQ/edit?usp=sharing
const GAS_PER_ORDER: u128 = 66315;
const GAS_PER_UNISWAP: u128 = 94696;

// TODO: exclude partially fillable orders
// TODO: set settlement.fee_factor
// TODO: special rounding for the prices we get from the solver?

/// The configuration passed as url parameters to the solver.
#[derive(Debug, Default)]
pub struct SolverConfig {
    pub max_nr_exec_orders: u32,
    pub time_limit: u32,
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
    native_token: H160,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
    network_id: String,
    fee_discount_factor: f64,
}

impl HttpSolver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base: Url,
        api_key: Option<String>,
        config: SolverConfig,
        native_token: H160,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        price_estimator: Arc<dyn PriceEstimating>,
        network_id: String,
        fee_discount_factor: f64,
    ) -> Self {
        // Unwrap because we cannot handle client creation failing.
        let client = Client::builder().build().unwrap();
        Self {
            base,
            client,
            api_key,
            config,
            native_token,
            token_info_fetcher,
            price_estimator,
            network_id,
            fee_discount_factor,
        }
    }

    // Solver api requires specifying token as strings. We use the address as a string for now.
    // Later we could use a more meaningful name like the token symbol but we have to ensure
    // uniqueness.
    fn token_to_string(&self, token: &H160) -> String {
        // Token names must start with a letter.
        format!("t{:x}", token)
    }

    fn map_tokens_for_solver(&self, orders: &[Liquidity]) -> HashMap<String, H160> {
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

    async fn token_models(
        &self,
        tokens: &HashMap<String, H160>,
        token_infos: &HashMap<H160, TokenInfo>,
        price_estimates: &HashMap<H160, Result<BigRational, PriceEstimationError>>,
    ) -> HashMap<String, TokenInfoModel> {
        tokens
            .iter()
            .map(|(index, address)| {
                let token_info = token_infos[address];
                let external_price = price_estimates[address]
                    .as_ref()
                    .ok()
                    .and_then(|price| price.to_f64());
                (
                    index.clone(),
                    TokenInfoModel {
                        decimals: token_info.decimals.map(|d| d as u32),
                        external_price,
                    },
                )
            })
            .collect()
    }

    fn map_orders_for_solver(&self, orders: Vec<LimitOrder>) -> HashMap<String, LimitOrder> {
        orders
            .into_iter()
            .enumerate()
            .map(|(index, order)| (index.to_string(), order))
            .collect()
    }

    async fn order_models(
        &self,
        orders: &HashMap<String, LimitOrder>,
        gas_price: f64,
    ) -> Result<HashMap<String, OrderModel>> {
        let order_cost = self.order_cost(gas_price).await;
        let mut result: HashMap<String, OrderModel> = HashMap::new();
        for (index, order) in orders {
            let order_fee = self.order_fee(&order)?;
            let order = OrderModel {
                sell_token: self.token_to_string(&order.sell_token),
                buy_token: self.token_to_string(&order.buy_token),
                sell_amount: order.sell_amount,
                buy_amount: order.buy_amount,
                allow_partial_fill: order.partially_fillable,
                is_sell_order: matches!(order.kind, OrderKind::Sell),
                fee: FeeModel {
                    amount: order_fee,
                    token: self.token_to_string(&order.sell_token),
                },
                cost: CostModel {
                    amount: order_cost,
                    token: self.token_to_string(&self.native_token),
                },
            };
            result.insert(index.clone(), order);
        }
        Ok(result)
    }

    fn map_amms_for_solver(&self, orders: Vec<AmmOrder>) -> HashMap<String, AmmOrder> {
        orders
            .into_iter()
            .enumerate()
            .map(|(index, amm)| (index.to_string(), amm))
            .collect()
    }

    async fn amm_models(
        &self,
        amms: &HashMap<String, AmmOrder>,
        gas_price: f64,
    ) -> HashMap<String, UniswapModel> {
        let uniswap_cost = self.uniswap_cost(gas_price).await;
        amms.iter()
            .map(|(index, amm)| {
                let uniswap = UniswapModel {
                    token1: self.token_to_string(&amm.tokens.get().0),
                    token2: self.token_to_string(&amm.tokens.get().1),
                    balance1: amm.reserves.0,
                    balance2: amm.reserves.1,
                    fee: *amm.fee.numer() as f64 / *amm.fee.denom() as f64,
                    cost: CostModel {
                        amount: uniswap_cost,
                        token: self.token_to_string(&self.native_token),
                    },
                    mandatory: false,
                };
                (index.clone(), uniswap)
            })
            .collect()
    }

    async fn prepare_model(
        &self,
        liquidity: Vec<Liquidity>,
        gas_price: f64,
    ) -> Result<(BatchAuctionModel, SettlementContext)> {
        // To send an instance to the solver we need to identify tokens and orders through strings.
        // In order to map back and forth we store the original tokens, orders and the models for
        // via the same mapping.
        let tokens = self.map_tokens_for_solver(liquidity.as_slice());

        let addresses: Vec<H160> = tokens.values().into_iter().cloned().collect();

        let (token_infos, price_estimates) = join!(
            self.token_info_fetcher
                .get_token_infos(addresses.as_slice()),
            self.price_estimator
                .estimate_prices(addresses.as_slice(), addresses[0])
        );

        let price_estimates: HashMap<H160, Result<BigRational, _>> =
            addresses.iter().cloned().zip(price_estimates).collect();

        let mut orders = split_liquidity(liquidity);

        // For the solver to run correctly we need to be sure that there are no isolated islands of
        // tokens without connection between them.
        remove_orders_without_native_connection(
            &mut orders.0,
            orders.1.as_slice(),
            &self.native_token,
        );
        let limit_orders = self.map_orders_for_solver(orders.0);
        let amm_orders = self.map_amms_for_solver(orders.1);
        let token_models = self
            .token_models(&tokens, &token_infos, &price_estimates)
            .await;
        let order_models = self.order_models(&limit_orders, gas_price).await?;
        let uniswap_models = self.amm_models(&amm_orders, gas_price).await;
        let model = BatchAuctionModel {
            tokens: token_models,
            orders: order_models,
            uniswaps: uniswap_models,
            metadata: Some(MetadataModel {
                environment: Some(self.network_id.clone()),
            }),
        };
        let context = SettlementContext {
            tokens,
            limit_orders,
            amm_orders,
        };
        Ok((model, context))
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
        tracing::trace!("request {}", body);
        let request = request.body(body.clone());
        let response = request.send().await.context("failed to send request")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to decode response body")?;
        tracing::trace!("response {}", text);
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

    async fn order_cost(&self, gas_price: f64) -> u128 {
        gas_price as u128 * GAS_PER_ORDER
    }

    async fn uniswap_cost(&self, gas_price: f64) -> u128 {
        gas_price as u128 * GAS_PER_UNISWAP
    }

    fn order_fee(&self, order: &LimitOrder) -> Result<u128> {
        (order.fee_amount.to_f64_lossy() / self.fee_discount_factor)
            .ceil()
            .to_u128()
            .context("failed to compute order fee")
    }
}

fn split_liquidity(liquidity: Vec<Liquidity>) -> (Vec<LimitOrder>, Vec<AmmOrder>) {
    let mut limit_orders = Vec::new();
    let mut amm_orders = Vec::new();
    for order in liquidity {
        match order {
            Liquidity::Limit(order) => limit_orders.push(order),
            Liquidity::Amm(order) => amm_orders.push(order),
        }
    }
    (limit_orders, amm_orders)
}

fn remove_orders_without_native_connection(
    orders: &mut Vec<LimitOrder>,
    amms: &[AmmOrder],
    native_token: &H160,
) {
    // Find all tokens that are connected through potentially multiple amm hops to the fee.
    // TODO: Replace with a more optimal graph algorithm.
    let mut amms = amms.iter().map(|amm| amm.tokens).collect::<HashSet<_>>();
    let mut fee_connected_tokens = std::iter::once(*native_token).collect::<HashSet<_>>();
    loop {
        let mut added_token = false;
        amms.retain(|token_pair| {
            let tokens = token_pair.get();
            if fee_connected_tokens.contains(&tokens.0) {
                fee_connected_tokens.insert(tokens.1);
                added_token = true;
                false
            } else if fee_connected_tokens.contains(&tokens.1) {
                fee_connected_tokens.insert(tokens.0);
                added_token = true;
                false
            } else {
                true
            }
        });
        if amms.is_empty() || !added_token {
            break;
        }
    }
    // Remove orders that are not connected.
    orders.retain(|order| {
        [order.buy_token, order.sell_token]
            .iter()
            .any(|token| fee_connected_tokens.contains(token))
    });
}

#[async_trait::async_trait]
impl Solver for HttpSolver {
    async fn solve(&self, liquidity: Vec<Liquidity>, gas_price: f64) -> Result<Vec<Settlement>> {
        let has_limit_orders = liquidity.iter().any(|l| matches!(l, Liquidity::Limit(_)));
        if !has_limit_orders {
            return Ok(Vec::new());
        };
        let (model, context) = self.prepare_model(liquidity, gas_price).await?;
        let settled = self.send(&model).await?;
        tracing::trace!(?settled);
        if !settled.has_execution_plan() {
            return Ok(Vec::new());
        }
        settlement::convert_settlement(settled, context).map(|settlement| vec![settlement])
    }

    fn name(&self) -> &'static str {
        "HTTPSolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{tests::CapturingSettlementHandler, AmmOrder, LimitOrder};
    use ::model::TokenPair;
    use maplit::hashmap;
    use num::rational::Ratio;
    use shared::price_estimate::mocks::FakePriceEstimator;
    use shared::token_info::MockTokenInfoFetching;
    use shared::token_info::TokenInfo;
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

        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_infos()
            .return_once(move |_| {
                hashmap! {
                    H160::zero() => TokenInfo { decimals: Some(18)},
                    H160::from_low_u64_be(1) => TokenInfo { decimals: Some(18)},
                }
            });
        let mock_token_info_fetcher: Arc<dyn TokenInfoFetching> = Arc::new(mock_token_info_fetcher);

        let mock_price_estimation: Arc<dyn PriceEstimating> =
            Arc::new(FakePriceEstimator(num::one()));

        let gas_price = 100.;

        let solver = HttpSolver::new(
            url.parse().unwrap(),
            None,
            SolverConfig {
                max_nr_exec_orders: 100,
                time_limit: 100,
            },
            H160::zero(),
            mock_token_info_fetcher,
            mock_price_estimation,
            "mock_network_id".to_string(),
            1.,
        );
        let base = |x: u128| x * 10u128.pow(18);
        let orders = vec![
            Liquidity::Limit(LimitOrder {
                buy_token: H160::zero(),
                sell_token: H160::from_low_u64_be(1),
                buy_amount: base(1).into(),
                sell_amount: base(2).into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                fee_amount: Default::default(),
                settlement_handling: CapturingSettlementHandler::arc(),
                id: "0".to_string(),
            }),
            Liquidity::Amm(AmmOrder {
                tokens: TokenPair::new(H160::zero(), H160::from_low_u64_be(1)).unwrap(),
                reserves: (base(100), base(100)),
                fee: Ratio::new(0, 1),
                settlement_handling: CapturingSettlementHandler::arc(),
            }),
        ];
        let (model, _context) = solver.prepare_model(orders, gas_price).await.unwrap();
        let settled = solver.send(&model).await.unwrap();
        dbg!(&settled);

        let exec_order = settled.orders.values().next().unwrap();
        assert_eq!(exec_order.exec_sell_amount.as_u128(), base(2));
        assert!(exec_order.exec_buy_amount.as_u128() > 0);

        let uniswap = settled.uniswaps.values().next().unwrap();
        assert!(uniswap.balance_update1 < 0);
        assert_eq!(uniswap.balance_update2 as u128, base(2));
        assert!(uniswap.exec_plan.is_some());
        assert_eq!(uniswap.exec_plan.as_ref().unwrap().sequence, 0);
        assert_eq!(uniswap.exec_plan.as_ref().unwrap().position, 0);

        assert_eq!(settled.prices.len(), 2);
    }

    #[test]
    fn remove_orders_without_native_connection_() {
        let limit_handling = CapturingSettlementHandler::arc();
        let amm_handling = CapturingSettlementHandler::arc();

        let native_token = H160::from_low_u64_be(0);
        let tokens = [
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
            H160::from_low_u64_be(3),
            H160::from_low_u64_be(4),
        ];

        let amms = [(native_token, tokens[0]), (tokens[0], tokens[1])]
            .iter()
            .map(|tokens| AmmOrder {
                tokens: TokenPair::new(tokens.0, tokens.1).unwrap(),
                reserves: (0, 0),
                fee: 0.into(),
                settlement_handling: amm_handling.clone(),
            })
            .collect::<Vec<_>>();

        let mut orders = [
            (native_token, tokens[0]),
            (native_token, tokens[1]),
            (tokens[0], tokens[1]),
            (tokens[1], tokens[0]),
            (tokens[1], tokens[2]),
            (tokens[2], tokens[1]),
            (tokens[2], tokens[3]),
            (tokens[3], tokens[2]),
        ]
        .iter()
        .map(|tokens| LimitOrder {
            sell_token: tokens.0,
            buy_token: tokens.1,
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            kind: OrderKind::Sell,
            partially_fillable: Default::default(),
            fee_amount: Default::default(),
            settlement_handling: limit_handling.clone(),
            id: "0".to_string(),
        })
        .collect::<Vec<_>>();

        remove_orders_without_native_connection(&mut orders, &amms, &native_token);
        assert_eq!(orders.len(), 6);
    }
}
