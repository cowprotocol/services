use crate::{settlement::Settlement, solver::Solver, uniswap};
use anyhow::{Context, Result};
use contracts::UniswapV2Factory;
use model::{
    order::{Order, OrderKind},
    u256_decimal,
};
use primitive_types::{H160, U256};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{hash_map::Entry, HashMap, HashSet};

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
    uniswap: UniswapV2Factory,
}

impl HttpSolver {
    pub fn new(
        base: Url,
        api_key: Option<String>,
        config: SolverConfig,
        uniswap: UniswapV2Factory,
    ) -> Self {
        // Unwrap because we cannot handle client creation failing.
        let client = Client::builder().build().unwrap();
        Self {
            base,
            client,
            api_key,
            config,
            uniswap,
        }
    }

    // Solver api requires specifying token as strings. We use the address as a string for now.
    // Later we could use a more meaningful name like the token symbol but we have to ensure
    // uniqueness.
    fn token_to_string(&self, token: &H160) -> String {
        // Token names must start with a letter.
        format!("t{:x}", token)
    }

    fn tokens(&self, orders: &[Order]) -> HashMap<String, TokenInfoModel> {
        orders
            .iter()
            .flat_map(|order| {
                let order = order.order_creation;
                std::iter::once(order.sell_token).chain(std::iter::once(order.buy_token))
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .map(|token| {
                // TODO: gather real decimals and store them in a cache
                let token_model = TokenInfoModel { decimals: 18 };
                (self.token_to_string(&token), token_model)
            })
            .collect()
    }

    fn orders(&self, orders: &[Order]) -> HashMap<String, OrderModel> {
        orders
            .iter()
            .enumerate()
            .map(|(index, order)| {
                let order = order.order_creation;
                let order = OrderModel {
                    sell_token: self.token_to_string(&order.sell_token),
                    buy_token: self.token_to_string(&order.buy_token),
                    sell_amount: order.sell_amount,
                    buy_amount: order.buy_amount,
                    allow_partial_fill: order.partially_fillable,
                    is_sell_order: matches!(order.kind, OrderKind::Sell),
                };
                (index.to_string(), order)
            })
            .collect()
    }

    async fn uniswaps(&self, orders: &[Order]) -> Result<HashMap<String, UniswapModel>> {
        // TODO: use a cache
        // TODO: include every token with ETH pair in the pools
        let mut uniswaps = HashMap::new();
        for order in orders {
            let pair = order.order_creation.token_pair().expect("invalid order");
            let vacant = match uniswaps.entry(pair) {
                Entry::Occupied(_) => continue,
                Entry::Vacant(vacant) => vacant,
            };
            let pool = match uniswap::Pool::from_token_pair(&self.uniswap, &pair)
                .await
                .context("failed to get uniswap pool")?
            {
                None => continue,
                Some(pool) => pool,
            };
            let uniswap = UniswapModel {
                token1: self.token_to_string(&pool.token_pair.get().0),
                token2: self.token_to_string(&pool.token_pair.get().1),
                balance1: pool.reserve0,
                balance2: pool.reserve1,
                fee: 0.003,
                mandatory: false,
            };
            vacant.insert(uniswap);
        }
        Ok(uniswaps
            .into_iter()
            .enumerate()
            .map(|(index, (_token_pair, uniswap))| (index.to_string(), uniswap))
            .collect())
    }

    async fn create_body(&self, orders: &[Order]) -> Result<BatchAuctionModel> {
        Ok(BatchAuctionModel {
            tokens: self.tokens(orders),
            orders: self.orders(orders),
            uniswaps: self.uniswaps(orders).await?,
            ref_token: self.token_to_string(&H160::zero()),
            default_fee: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl Solver for HttpSolver {
    async fn solve(&self, orders: Vec<Order>) -> Result<Option<Settlement>> {
        let mut url = self.base.clone();
        url.set_path("/solve");
        self.config.add_to_query(&mut url);
        let body = self.create_body(orders.as_slice()).await?;
        let mut request = self.client.post(url);
        if let Some(api_key) = &self.api_key {
            request = request.header("X-API-KEY", api_key);
        }
        let request = request.json(&body);

        let response = request
            .send()
            .await
            .context("failed to send solver request")?
            .error_for_status()
            .context("solver response was unsuccessful")?;
        let body = response
            .bytes()
            .await
            .context("failed to get response body")?;
        let _decoded: Solution =
            serde_json::from_slice(&body).with_context(|| match std::str::from_utf8(&body) {
                Ok(body) => format!("failed to decode response body: {}", body),
                Err(_) => format!("failed to decode response body: {:?}", body),
            })?;
        Ok(None)
    }
}

// types used in the solver http api

#[derive(Debug, Default, Serialize)]
struct BatchAuctionModel {
    tokens: HashMap<String, TokenInfoModel>,
    orders: HashMap<String, OrderModel>,
    uniswaps: HashMap<String, UniswapModel>,
    ref_token: String,
    #[serde(serialize_with = "serialize_as_string")]
    default_fee: f32,
}

#[derive(Debug, Deserialize)]
struct Solution {
    // TODO: wait for solution format to be documented
}

#[derive(Debug, Serialize)]
struct OrderModel {
    sell_token: String,
    buy_token: String,
    #[serde(with = "u256_decimal")]
    sell_amount: U256,
    #[serde(with = "u256_decimal")]
    buy_amount: U256,
    allow_partial_fill: bool,
    is_sell_order: bool,
}

#[derive(Debug, Serialize)]
struct UniswapModel {
    token1: String,
    token2: String,
    #[serde(serialize_with = "serialize_as_string")]
    balance1: u128,
    #[serde(serialize_with = "serialize_as_string")]
    balance2: u128,
    #[serde(serialize_with = "serialize_as_string")]
    fee: f32,
    mandatory: bool,
}

#[derive(Debug, Serialize)]
struct TokenInfoModel {
    #[serde(serialize_with = "serialize_as_string")]
    decimals: u32,
}

fn serialize_as_string<S>(t: &impl ToString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(t.to_string().as_str())
}

#[cfg(test)]
mod tests {
    use model::order::{OrderCreation, OrderMetaData};

    use super::*;

    // cargo test real_solver -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_solver() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter("debug")
            .init();
        let node_url = "https://dev-openethereum.mainnet.gnosisdev.com";
        let transport = web3::transports::Http::new(node_url).unwrap();
        let web3 = web3::Web3::new(transport);
        let uniswap = contracts::UniswapV2Factory::deployed(&web3).await.unwrap();
        let solver = HttpSolver::new(
            "http://localhost:8000".parse().unwrap(),
            None,
            SolverConfig {
                max_nr_exec_orders: 100,
                time_limit: 100,
            },
            uniswap,
        );
        let orders = vec![Order {
            order_meta_data: OrderMetaData::default(),
            order_creation: OrderCreation {
                sell_token: H160::from_low_u64_be(1),
                buy_amount: 1.into(),
                sell_amount: 1.into(),
                ..Default::default()
            },
        }];
        solver.solve(orders).await.unwrap();
    }
}
