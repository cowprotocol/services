pub mod buffers;
pub mod model;
mod settlement;

use self::{model::*, settlement::SettlementContext};
use crate::{
    liquidity::{
        ConstantProductOrder, LimitOrder, Liquidity, StablePoolOrder, WeightedProductOrder,
    },
    settlement::Settlement,
    settlement_submission::retry::is_transaction_failure,
    solver::{Auction, Solver},
};
use ::model::order::OrderKind;
use anyhow::{anyhow, ensure, Context, Result};
use bigdecimal::ToPrimitive;
use buffers::{BufferRetrievalError, BufferRetrieving};
use ethcontract::{Account, U256};
use futures::{join, lock::Mutex};
use lazy_static::lazy_static;
use num::{BigInt, BigRational};
use primitive_types::H160;
use reqwest::{header::HeaderValue, Client, Url};
use shared::{
    measure_time,
    price_estimate::{self, PriceEstimating},
    token_info::{TokenInfo, TokenInfoFetching},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

// Estimates from multivariate linear regression here:
// https://docs.google.com/spreadsheets/d/13UeUQ9DA4bHlcy9-i8d4nSLlCxSfjcXpTelvXYzyJzQ/edit?usp=sharing
lazy_static! {
    static ref GAS_PER_ORDER: U256 = U256::from(66_315);
    static ref GAS_PER_UNISWAP: U256 = U256::from(94_696);
    // Taken from a sample of two swaps
    // https://etherscan.io/tx/0x72d234d35fd169ef497ba0a1dc23258c96f278fb688d375d135eb012e5311009
    // https://etherscan.io/tx/0x1c345a6da1edb2bba953685a4cf85f6a0d967ac751f8c5b518578c5fd20a7c96
    static ref GAS_PER_BALANCER_SWAP: U256 = U256::from(120_000);
}

// TODO: exclude partially fillable orders
// TODO: set settlement.fee_factor
// TODO: special rounding for the prices we get from the solver?

/// The configuration passed as url parameters to the solver.
#[derive(Debug, Default)]
pub struct SolverConfig {
    pub max_nr_exec_orders: u32,
}

impl SolverConfig {
    fn add_to_query(&self, url: &mut Url) {
        url.query_pairs_mut().append_pair(
            "max_nr_exec_orders",
            self.max_nr_exec_orders.to_string().as_str(),
        );
    }
}

/// Data shared between multiple instances of the http solver for the same solve id.
pub struct InstanceData {
    solve_id: u64,
    model: BatchAuctionModel,
    context: SettlementContext,
}

/// We keep a cache of per solve instance data because it is the same for all http solver
/// invocations. Without the cache we would duplicate most of the requests to the node.
pub type InstanceCache = Arc<Mutex<Option<InstanceData>>>;

pub struct HttpSolver {
    name: &'static str,
    account: Account,
    base: Url,
    client: Client,
    api_key: Option<String>,
    config: SolverConfig,
    native_token: H160,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
    buffer_retriever: Arc<dyn BufferRetrieving>,
    network_id: String,
    chain_id: u64,
    instance_cache: InstanceCache,
    fee_factor: f64,
    native_token_amount_to_estimate_prices_with: U256,
}

impl HttpSolver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &'static str,
        account: Account,
        base: Url,
        api_key: Option<String>,
        config: SolverConfig,
        native_token: H160,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        price_estimator: Arc<dyn PriceEstimating>,
        buffer_retriever: Arc<dyn BufferRetrieving>,
        network_id: String,
        chain_id: u64,
        fee_factor: f64,
        client: Client,
        instance_cache: InstanceCache,
        native_token_amount_to_estimate_prices_with: U256,
    ) -> Self {
        Self {
            name,
            account,
            base,
            client,
            api_key,
            config,
            native_token,
            token_info_fetcher,
            price_estimator,
            buffer_retriever,
            network_id,
            chain_id,
            instance_cache,
            fee_factor,
            native_token_amount_to_estimate_prices_with,
        }
    }

    fn map_tokens_for_solver(&self, orders: &[LimitOrder], liquidity: &[Liquidity]) -> Vec<H160> {
        let order_tokens = orders
            .iter()
            .flat_map(|order| [order.sell_token, order.buy_token]);
        let liquidity_tokens = liquidity.iter().flat_map(|liquidity| match liquidity {
            Liquidity::ConstantProduct(amm) => amm.tokens.into_iter().collect::<Vec<_>>(),
            Liquidity::BalancerWeighted(amm) => amm.reserves.keys().copied().collect(),
            Liquidity::BalancerStable(amm) => amm.reserves.keys().copied().collect(),
        });

        order_tokens
            .chain(liquidity_tokens)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn token_models(
        &self,
        token_infos: &HashMap<H160, TokenInfo>,
        price_estimates: &HashMap<H160, f64>,
        buffers: &HashMap<H160, U256>,
    ) -> HashMap<H160, TokenInfoModel> {
        token_infos
            .iter()
            .map(|(address, token_info)| {
                let external_price = match price_estimates.get(address).copied() {
                    Some(price) if price.is_finite() => Some(price),
                    _ => None,
                };
                (
                    *address,
                    TokenInfoModel {
                        decimals: token_info.decimals,
                        external_price,
                        normalize_priority: Some(if &self.native_token == address { 1 } else { 0 }),
                        internal_buffer: buffers.get(address).copied(),
                    },
                )
            })
            .collect()
    }

    fn map_orders_for_solver(&self, orders: Vec<LimitOrder>) -> HashMap<usize, LimitOrder> {
        orders.into_iter().enumerate().collect()
    }

    fn order_models(
        &self,
        orders: &HashMap<usize, LimitOrder>,
        gas_price: f64,
    ) -> HashMap<usize, OrderModel> {
        let order_cost = self.order_cost(gas_price);
        let mut result: HashMap<usize, OrderModel> = HashMap::new();
        for (index, order) in orders {
            let order_fee = self.order_fee(order);
            let order = OrderModel {
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                sell_amount: order.sell_amount,
                buy_amount: order.buy_amount,
                allow_partial_fill: order.partially_fillable,
                is_sell_order: matches!(order.kind, OrderKind::Sell),
                fee: FeeModel {
                    amount: order_fee,
                    token: order.sell_token,
                },
                cost: CostModel {
                    amount: order_cost,
                    token: self.native_token,
                },
                is_liquidity_order: order.is_liquidity_order,
            };
            result.insert(*index, order);
        }
        result
    }

    fn map_amm_orders_for_solver<T>(&self, orders: Vec<T>) -> HashMap<usize, T> {
        orders.into_iter().enumerate().collect()
    }

    fn amm_models(
        &self,
        constant_product_orders: &HashMap<usize, ConstantProductOrder>,
        weighted_product_orders: &HashMap<usize, WeightedProductOrder>,
        stable_pool_orders: &HashMap<usize, StablePoolOrder>,
        gas_price: f64,
    ) -> HashMap<usize, AmmModel> {
        let uniswap_cost = self.uniswap_cost(gas_price);
        let mut pool_model_map = HashMap::new();
        let constant_product_models: HashMap<_, AmmModel> = constant_product_orders
            .iter()
            .map(|(index, amm)| {
                let mut reserves = HashMap::new();
                reserves.insert(amm.tokens.get().0, U256::from(amm.reserves.0));
                reserves.insert(amm.tokens.get().1, U256::from(amm.reserves.1));
                let pool_model = AmmModel {
                    parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                        reserves,
                    }),
                    fee: BigRational::new(
                        BigInt::from(*amm.fee.numer()),
                        BigInt::from(*amm.fee.denom()),
                    ),
                    cost: CostModel {
                        amount: uniswap_cost,
                        token: self.native_token,
                    },
                    mandatory: false,
                };
                (*index, pool_model)
            })
            .collect();
        let balancer_cost = self.balancer_cost(gas_price);
        let weighted_product_models: HashMap<_, AmmModel> = weighted_product_orders
            .iter()
            .map(|(index, amm)| {
                let reserves = amm
                    .reserves
                    .iter()
                    .map(|(token, state)| {
                        (
                            *token,
                            WeightedPoolTokenData {
                                balance: state.token_state.balance,
                                weight: BigRational::from(state.weight),
                            },
                        )
                    })
                    .collect();
                let pool_model = AmmModel {
                    parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                        reserves,
                    }),
                    fee: amm.fee.clone(),
                    cost: CostModel {
                        amount: balancer_cost,
                        token: self.native_token,
                    },
                    mandatory: false,
                };
                // Note that in order to preserve unique keys of this hashmap, we use
                // the current index + the length of the previous map.
                (*index + constant_product_models.len(), pool_model)
            })
            .collect();
        let stable_pool_models: HashMap<_, AmmModel> = stable_pool_orders
            .iter()
            .map(|(index, amm)| {
                let reserves = amm
                    .reserves
                    .iter()
                    .map(|(token, state)| (*token, state.balance))
                    .collect();
                let pool_model = AmmModel {
                    parameters: AmmParameters::Stable(StablePoolParameters {
                        reserves,
                        amplification_parameter: amm.amplification_parameter.clone(),
                    }),
                    fee: amm.fee.clone(),
                    cost: CostModel {
                        amount: balancer_cost,
                        token: self.native_token,
                    },
                    mandatory: false,
                };
                // Note that in order to preserve unique keys of this hashmap, we use
                // the current index + the length of the previous map.
                (
                    *index + constant_product_models.len() + weighted_product_models.len(),
                    pool_model,
                )
            })
            .collect();
        pool_model_map.extend(constant_product_models);
        pool_model_map.extend(weighted_product_models);
        pool_model_map.extend(stable_pool_models);
        pool_model_map
    }

    async fn prepare_model(
        &self,
        mut orders: Vec<LimitOrder>,
        liquidity: Vec<Liquidity>,
        gas_price: f64,
        price_estimates: HashMap<H160, BigRational>,
    ) -> Result<(BatchAuctionModel, SettlementContext)> {
        let tokens = self.map_tokens_for_solver(&orders, &liquidity);

        let queries = tokens
            .iter()
            .filter(|token| !price_estimates.contains_key(token))
            .map(|token| price_estimate::Query {
                sell_token: self.native_token,
                buy_token: *token,
                in_amount: self.native_token_amount_to_estimate_prices_with,
                kind: OrderKind::Sell,
            })
            .collect::<Vec<_>>();

        let (token_infos, new_price_estimates, mut buffers_result) = join!(
            measure_time(
                self.token_info_fetcher.get_token_infos(tokens.as_slice()),
                |duration| tracing::debug!("get_token_infos took {} s", duration.as_secs_f32()),
            ),
            measure_time(
                self.price_estimator.estimates(&queries),
                |duration| tracing::debug!("estimate_prices took {} s", duration.as_secs_f32()),
            ),
            measure_time(
                self.buffer_retriever.get_buffers(tokens.as_slice()),
                |duration| tracing::debug!("get_buffers took {} s", duration.as_secs_f32()),
            ),
        );

        let buffers: HashMap<_, _> = buffers_result
            .drain()
            .filter_map(|(token, buffer)| match buffer {
                Err(BufferRetrievalError::Erc20(err)) if is_transaction_failure(&err.inner) => {
                    tracing::debug!(
                        "Failed to fetch buffers for token {} with transaction failure {}",
                        token,
                        err
                    );
                    None
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to fetch buffers contract balance for token {} with error {:?}",
                        token,
                        err
                    );
                    None
                }
                Ok(b) => Some((token, b)),
            })
            .collect();

        let mut new_price_estimates: HashMap<H160, f64> = queries
            .iter()
            .zip(new_price_estimates)
            .filter_map(|(query, estimate)| {
                let price = estimate.ok()?.price_in_sell_token_f64(query);
                Some((query.buy_token, price))
            })
            .collect();
        new_price_estimates.extend(
            price_estimates
                .into_iter()
                .filter_map(|(token, price)| Some((token, price.to_f64()?))),
        );

        let (constant_product_liquidity, weighted_product_liquidity, stable_pool_liquidity) =
            split_liquidity(liquidity);

        // For the solver to run correctly we need to be sure that there are no isolated islands of
        // tokens without connection between them.
        remove_orders_without_native_connection(
            &mut orders,
            &constant_product_liquidity,
            &self.native_token,
        );
        let limit_orders = self.map_orders_for_solver(orders);
        let constant_product_orders = self.map_amm_orders_for_solver(constant_product_liquidity);
        let weighted_product_orders = self.map_amm_orders_for_solver(weighted_product_liquidity);
        let stable_pool_orders = self.map_amm_orders_for_solver(stable_pool_liquidity);

        let token_models = self.token_models(&token_infos, &new_price_estimates, &buffers);
        let order_models = self.order_models(&limit_orders, gas_price);
        let amm_models = self
            .amm_models(
                &constant_product_orders,
                &weighted_product_orders,
                &stable_pool_orders,
                gas_price,
            )
            .into_iter()
            .filter(|(_, model)| model.has_sufficient_reserves())
            .collect();
        let model = BatchAuctionModel {
            tokens: token_models,
            orders: order_models,
            amms: amm_models,
            metadata: Some(MetadataModel {
                environment: Some(self.network_id.clone()),
            }),
        };
        let context = SettlementContext {
            limit_orders,
            constant_product_orders,
            weighted_product_orders,
            stable_pool_orders,
        };
        Ok((model, context))
    }

    async fn send(
        &self,
        model: &BatchAuctionModel,
        deadline: Instant,
    ) -> Result<SettledBatchAuctionModel> {
        // The timeout we give to the solver is one second less than the deadline to make up for
        // overhead from the network.
        let timeout_buffer = Duration::from_secs(1);
        let timeout = deadline
            .checked_duration_since(Instant::now() + timeout_buffer)
            .ok_or_else(|| anyhow!("no time left to send request"))?;

        let mut url = self.base.clone();
        url.set_path("/solve");

        let instance_name = self.generate_instance_name();
        tracing::info!("http solver instance name is {}", instance_name);
        url.query_pairs_mut()
            .append_pair("instance_name", &instance_name)
            .append_pair("time_limit", &timeout.as_secs().to_string());

        self.config.add_to_query(&mut url);
        let query = url.query().map(ToString::to_string).unwrap_or_default();
        // The default Client created in main has a short http request timeout which we might
        // exceed. We remove it here because the solver already internally enforces the timeout and
        // the driver code that calls us also enforces the timeout.
        let mut request = self.client.post(url).timeout(Duration::from_secs(u64::MAX));
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

    fn order_cost(&self, gas_price: f64) -> U256 {
        U256::from_f64_lossy(gas_price) * *GAS_PER_ORDER
    }

    fn uniswap_cost(&self, gas_price: f64) -> U256 {
        U256::from_f64_lossy(gas_price) * *GAS_PER_UNISWAP
    }

    fn balancer_cost(&self, gas_price: f64) -> U256 {
        U256::from_f64_lossy(gas_price) * *GAS_PER_BALANCER_SWAP
    }

    fn order_fee(&self, order: &LimitOrder) -> U256 {
        let ceiled_div = (order.fee_amount.to_f64_lossy() / self.fee_factor).ceil();
        U256::from_f64_lossy(ceiled_div)
    }

    pub fn generate_instance_name(&self) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}_{}_{}",
            now.to_string().replace(" ", "_"),
            self.network_id,
            self.chain_id
        )
    }
}

fn split_liquidity(
    liquidity: Vec<Liquidity>,
) -> (
    Vec<ConstantProductOrder>,
    Vec<WeightedProductOrder>,
    Vec<StablePoolOrder>,
) {
    let mut constant_product_orders = Vec::new();
    let mut weighted_product_orders = Vec::new();
    let mut stable_pool_orders = Vec::new();
    for order in liquidity {
        match order {
            Liquidity::ConstantProduct(order) => constant_product_orders.push(order),
            Liquidity::BalancerWeighted(order) => weighted_product_orders.push(order),
            Liquidity::BalancerStable(order) => stable_pool_orders.push(order),
        }
    }
    (
        constant_product_orders,
        weighted_product_orders,
        stable_pool_orders,
    )
}

// TODO: This currently does **NOT** consider balancer pools, but definitely should.
fn remove_orders_without_native_connection(
    orders: &mut Vec<LimitOrder>,
    amms: &[ConstantProductOrder],
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
    async fn solve(
        &self,
        Auction {
            id,
            orders,
            liquidity,
            gas_price,
            deadline,
            price_estimates,
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        };
        let (model, context) = {
            let mut guard = self.instance_cache.lock().await;
            match guard.as_mut() {
                Some(data) if data.solve_id == id => (data.model.clone(), data.context.clone()),
                _ => {
                    let (model, context) = self
                        .prepare_model(orders, liquidity, gas_price, price_estimates)
                        .await?;
                    *guard = Some(InstanceData {
                        solve_id: id,
                        model: model.clone(),
                        context: context.clone(),
                    });
                    (model, context)
                }
            }
        };
        let settled = self.send(&model, deadline).await?;
        tracing::trace!(?settled);
        if !settled.has_execution_plan() {
            return Ok(Vec::new());
        }
        settlement::convert_settlement(settled, context).map(|settlement| vec![settlement])
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{tests::CapturingSettlementHandler, ConstantProductOrder, LimitOrder};
    use crate::solver::http_solver::buffers::MockBufferRetrieving;
    use ::model::TokenPair;
    use ethcontract::Address;
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

        let buy_token = H160::from_low_u64_be(1337);
        let sell_token = H160::from_low_u64_be(43110);

        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_infos()
            .return_once(move |_| {
                hashmap! {
                    buy_token => TokenInfo { decimals: Some(18)},
                    sell_token => TokenInfo { decimals: Some(18)},
                }
            });
        let mock_token_info_fetcher: Arc<dyn TokenInfoFetching> = Arc::new(mock_token_info_fetcher);

        let mut mock_buffer_retriever = MockBufferRetrieving::new();
        mock_buffer_retriever
            .expect_get_buffers()
            .return_once(move |_| {
                hashmap! {
                    buy_token => Ok(U256::from(42)),
                    sell_token => Ok(U256::from(1337)),
                }
            });
        let mock_buffer_retriever: Arc<dyn BufferRetrieving> = Arc::new(mock_buffer_retriever);

        let mock_price_estimation: Arc<dyn PriceEstimating> =
            Arc::new(FakePriceEstimator(price_estimate::Estimate {
                out_amount: 1.into(),
                gas: 1.into(),
            }));

        let gas_price = 100.;

        let solver = HttpSolver::new(
            "Test Solver",
            Account::Local(Address::default(), None),
            url.parse().unwrap(),
            None,
            SolverConfig {
                max_nr_exec_orders: 100,
            },
            H160::zero(),
            mock_token_info_fetcher,
            mock_price_estimation,
            mock_buffer_retriever,
            "mock_network_id".to_string(),
            0,
            1.,
            Client::new(),
            Default::default(),
            1.into(),
        );
        let base = |x: u128| x * 10u128.pow(18);
        let limit_orders = vec![LimitOrder {
            buy_token,
            sell_token,
            buy_amount: base(1).into(),
            sell_amount: base(2).into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            is_liquidity_order: false,
            id: "0".to_string(),
        }];
        let liquidity = vec![Liquidity::ConstantProduct(ConstantProductOrder {
            tokens: TokenPair::new(buy_token, sell_token).unwrap(),
            reserves: (base(100), base(100)),
            fee: Ratio::new(0, 1),
            settlement_handling: CapturingSettlementHandler::arc(),
        })];
        let (model, _context) = solver
            .prepare_model(limit_orders, liquidity, gas_price, Default::default())
            .await
            .unwrap();
        let settled = solver
            .send(&model, Instant::now() + Duration::from_secs(1000))
            .await
            .unwrap();
        dbg!(&settled);

        let exec_order = settled.orders.values().next().unwrap();
        assert_eq!(exec_order.exec_sell_amount.as_u128(), base(2));
        assert!(exec_order.exec_buy_amount.as_u128() > 0);

        let uniswap = settled.amms.values().next().unwrap();
        let execution = &uniswap.execution[0];
        assert!(execution.exec_buy_amount.gt(&U256::zero()));
        assert_eq!(execution.exec_sell_amount, U256::from(base(2)));
        assert!(execution.exec_plan.is_some());
        assert_eq!(execution.exec_plan.as_ref().unwrap().sequence, 0);
        assert_eq!(execution.exec_plan.as_ref().unwrap().position, 0);

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
            .map(|tokens| ConstantProductOrder {
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
            is_liquidity_order: false,
            id: "0".to_string(),
        })
        .collect::<Vec<_>>();

        remove_orders_without_native_connection(&mut orders, &amms, &native_token);
        assert_eq!(orders.len(), 6);
    }

    #[test]
    fn decode_response() {
        let example_response = r#"
            {
              "extra_crap": ["Hello"],
              "orders": {
                "0": {
                  "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                  "buy_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                  "sell_amount": "195160000000000000",
                  "buy_amount": "18529625032931383084",
                  "allow_partial_fill": false,
                  "is_sell_order": true,
                  "fee": {
                    "amount": "4840000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "cost": {
                    "amount": "1604823000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "exec_buy_amount": "18689825362370811941",
                  "exec_sell_amount": "195160000000000000"
                },
                "1": {
                  "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                  "buy_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                  "sell_amount": "395160000000000000",
                  "buy_amount": "37314737669229514851",
                  "allow_partial_fill": false,
                  "is_sell_order": true,
                  "fee": {
                    "amount": "4840000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "cost": {
                    "amount": "1604823000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "exec_buy_amount": "37843161458262200293",
                  "exec_sell_amount": "395160000000000000"
                }
              },
              "ref_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
              "prices": {
                "0xba100000625a3754423978a60c9317c58a424e3d": "10442045135045813",
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000"
              },
              "amms": {
                "9": {
                  "kind": "WeightedProduct",
                  "reserves": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                      "balance": "99572200495363891220",
                      "weight": "0.5"
                    },
                    "0xba100000625a3754423978a60c9317c58a424e3d": {
                      "balance": "9605600791222732320384",
                      "weight": "0.5"
                    }
                  },
                  "fee": "0.0014",
                  "cost": {
                    "amount": "2904000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "execution": [
                    {
                      "sell_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                      "buy_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                      "exec_sell_amount": "56532986820633012234",
                      "exec_buy_amount": "590320000000000032",
                      "exec_plan": {
                        "sequence": 0,
                        "position": 0
                      }
                    }
                  ]
                }
              }
            }
        "#;
        let parsed_response = serde_json::from_str::<SettledBatchAuctionModel>(example_response);
        assert!(parsed_response.is_ok());
    }
}
