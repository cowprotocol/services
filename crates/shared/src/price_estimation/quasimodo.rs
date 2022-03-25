use crate::{
    baseline_solver::BaseTokens,
    http_solver::{
        gas_model::GasModel,
        model::{
            AmmModel, AmmParameters, BatchAuctionModel, ConstantProductPoolParameters, CostModel,
            FeeModel, OrderModel, SettledBatchAuctionModel, StablePoolParameters, TokenInfoModel,
            WeightedPoolTokenData, WeightedProductPoolParameters,
        },
        HttpSolverApi,
    },
    price_estimation::{
        gas::{ERC20_TRANSFER, GAS_PER_ORDER, INITIALIZATION_COST, SETTLEMENT},
        Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
    recent_block_cache::Block,
    request_sharing::RequestSharing,
    sources::{
        balancer_v2::{
            pools::common::compute_scaling_rate, BalancerPoolFetcher, BalancerPoolFetching,
        },
        uniswap_v2::{pool_cache::PoolCache, pool_fetching::PoolFetching},
    },
    token_info::TokenInfoFetching,
};
use anyhow::{Context, Result};
use ethcontract::{H160, U256};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use gas_estimation::GasPriceEstimating;
use model::{order::OrderKind, TokenPair};
use num::{BigInt, BigRational};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};

pub struct QuasimodoPriceEstimator {
    api: Arc<dyn HttpSolverApi>,
    sharing: RequestSharing<
        Query,
        BoxFuture<'static, Result<SettledBatchAuctionModel, PriceEstimationError>>,
    >,
    pools: Arc<PoolCache>,
    balancer_pools: Option<Arc<BalancerPoolFetcher>>,
    token_info: Arc<dyn TokenInfoFetching>,
    gas_info: Arc<dyn GasPriceEstimating>,
    native_token: H160,
    base_tokens: Arc<BaseTokens>,
}

impl QuasimodoPriceEstimator {
    pub fn new(
        api: Arc<dyn HttpSolverApi>,
        pools: Arc<PoolCache>,
        balancer_pools: Option<Arc<BalancerPoolFetcher>>,
        token_info: Arc<dyn TokenInfoFetching>,
        gas_info: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        base_tokens: Arc<BaseTokens>,
    ) -> Self {
        Self {
            api,
            sharing: Default::default(),
            pools,
            balancer_pools,
            token_info,
            gas_info,
            native_token,
            base_tokens,
        }
    }

    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        let gas_price = U256::from_f64_lossy(self.gas_info.estimate().await?.effective_gas_price());

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (U256::max_value(), query.in_amount),
            OrderKind::Sell => (query.in_amount, U256::one()),
        };

        let orders = maplit::btreemap! {
            0 => OrderModel {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                sell_amount,
                buy_amount,
                allow_partial_fill: false,
                is_sell_order: query.kind == OrderKind::Sell,
                fee: FeeModel {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                cost: CostModel {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                is_liquidity_order: false,
                mandatory: true,
                has_atomic_execution: false,
            },
        };

        let token_pair = TokenPair::new(query.sell_token, query.buy_token).unwrap();
        let pairs = self.base_tokens.relevant_pairs([token_pair].into_iter());
        let gas_model = GasModel {
            native_token: self.native_token,
            gas_price: gas_price.to_f64_lossy(),
        };

        let (uniswap_pools, balancer_pools) = futures::try_join!(
            self.uniswap_pools(pairs.clone(), &gas_model),
            self.balancer_pools(pairs.clone(), &gas_model)
        )?;
        let amms: BTreeMap<usize, AmmModel> = uniswap_pools
            .into_iter()
            .chain(balancer_pools)
            .enumerate()
            .collect();

        let mut tokens: HashSet<H160> = Default::default();
        tokens.insert(query.sell_token);
        tokens.insert(query.buy_token);
        tokens.insert(self.native_token);
        for amm in amms.values() {
            match &amm.parameters {
                AmmParameters::ConstantProduct(params) => tokens.extend(params.reserves.keys()),
                AmmParameters::WeightedProduct(params) => tokens.extend(params.reserves.keys()),
                AmmParameters::Stable(params) => tokens.extend(params.reserves.keys()),
            }
        }
        let tokens: Vec<_> = tokens.drain().collect();
        let token_infos = self.token_info.get_token_infos(&tokens).await;
        let tokens = tokens
            .iter()
            .map(|token| {
                let info = token_infos.get(token).cloned().unwrap_or_default();
                (
                    *token,
                    TokenInfoModel {
                        decimals: info.decimals,
                        alias: info.symbol,
                        normalize_priority: Some(if *token == self.native_token { 1 } else { 0 }),
                        ..Default::default()
                    },
                )
            })
            .collect();

        let model = BatchAuctionModel {
            tokens,
            orders,
            amms,
            metadata: None,
        };

        let api = self.api.clone();
        let settlement_future = async move {
            api.solve(
                &model,
                // We need at least three seconds of timeout. Quasimodo
                // reserves one second of timeout for shutdown, plus one
                // more second is reserved for network interactions.
                Duration::from_secs(3),
            )
            .await
            .map_err(PriceEstimationError::Other)
        };
        let settlement = self
            .sharing
            .shared(*query, settlement_future.boxed())
            .await?;

        if !settlement.orders.contains_key(&0) {
            return Err(PriceEstimationError::NoLiquidity);
        }

        let mut cost = self.extract_cost(&settlement.orders[&0].cost)?;
        for amm in settlement.amms.values() {
            cost += self.extract_cost(&amm.cost)? * amm.execution.len();
        }
        let gas = (cost / gas_price).as_u64()
            + INITIALIZATION_COST // Call into contract
            + SETTLEMENT // overhead for entering the `settle()` function
            + ERC20_TRANSFER * 2; // transfer in and transfer out

        Ok(Estimate {
            out_amount: match query.kind {
                OrderKind::Buy => settlement.orders[&0].exec_sell_amount,
                OrderKind::Sell => settlement.orders[&0].exec_buy_amount,
            },
            gas,
        })
    }

    async fn uniswap_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = self
            .pools
            .fetch(pairs, Block::Recent)
            .await
            .context("pools")?;
        Ok(pools
            .into_iter()
            .map(|pool| AmmModel {
                parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                    reserves: BTreeMap::from([
                        (pool.tokens.get().0, pool.reserves.0.into()),
                        (pool.tokens.get().1, pool.reserves.1.into()),
                    ]),
                }),
                fee: BigRational::from((
                    BigInt::from(*pool.fee.numer()),
                    BigInt::from(*pool.fee.denom()),
                )),
                cost: gas_model.uniswap_cost(),
                mandatory: false,
            })
            .collect())
    }

    async fn balancer_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = match &self.balancer_pools {
            Some(balancer) => balancer
                .fetch(pairs, Block::Recent)
                .await
                .context("balancer_pools")?,
            None => return Ok(Vec::new()),
        };
        // There is some code duplication between here and crates/solver/src/solver/http_solver.rs  fn amm_models .
        // To avoid that we would need to make both components work on the same input balancer
        // types. Currently solver uses a liquidity type that is specific to the solver crate.
        let weighted = pools.weighted_pools.into_iter().map(|pool| AmmModel {
            parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                reserves: pool
                    .reserves
                    .into_iter()
                    .map(|(token, state)| {
                        (
                            token,
                            WeightedPoolTokenData {
                                balance: state.common.balance,
                                weight: BigRational::from(state.weight),
                            },
                        )
                    })
                    .collect(),
            }),
            fee: pool.common.swap_fee.into(),
            cost: gas_model.balancer_cost(),
            mandatory: false,
        });
        let stable = pools
            .stable_pools
            .into_iter()
            .map(|pool| -> Result<AmmModel> {
                Ok(AmmModel {
                    parameters: AmmParameters::Stable(StablePoolParameters {
                        reserves: pool
                            .reserves
                            .iter()
                            .map(|(token, state)| (*token, state.balance))
                            .collect(),
                        scaling_rates: pool
                            .reserves
                            .into_iter()
                            .map(|(token, state)| {
                                Ok((token, compute_scaling_rate(state.scaling_exponent)?))
                            })
                            .collect::<Result<_>>()
                            .with_context(|| "convert stable pool to solver model".to_string())?,
                        amplification_parameter: pool.amplification_parameter.as_big_rational(),
                    }),
                    fee: pool.common.swap_fee.into(),
                    cost: gas_model.balancer_cost(),
                    mandatory: false,
                })
            });
        let mut models = Vec::from_iter(weighted);
        for stable in stable {
            models.push(stable?);
        }
        Ok(models)
    }

    fn extract_cost(&self, cost: &Option<CostModel>) -> Result<U256, PriceEstimationError> {
        if let Some(cost) = cost {
            if cost.token != self.native_token {
                Err(anyhow::anyhow!("cost specified as an unknown token {}", cost.token).into())
            } else {
                Ok(cost.amount)
            }
        } else {
            Ok(U256::zero())
        }
    }
}

impl PriceEstimating for QuasimodoPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(query))
            .enumerate()
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::current_block::current_block_stream;
    use crate::http_solver::{DefaultHttpSolverApi, SolverConfig};
    use crate::price_estimation::Query;
    use crate::recent_block_cache::CacheConfig;
    use crate::sources::balancer_v2::pool_fetching::BalancerContracts;
    use crate::sources::balancer_v2::BalancerFactoryKind;
    use crate::sources::uniswap_v2;
    use crate::sources::uniswap_v2::pool_cache::NoopPoolCacheMetrics;
    use crate::token_info::TokenInfoFetcher;
    use crate::transport::http::HttpTransport;
    use crate::Web3;
    use clap::ArgEnum;
    use ethcontract::dyns::DynTransport;
    use model::order::OrderKind;
    use reqwest::Client;
    use url::Url;

    // TODO: to implement these tests, we'll need to make HTTP solver API mockable.

    #[tokio::test]
    async fn estimate_sell() {}

    #[tokio::test]
    async fn estimate_buy() {}

    #[tokio::test]
    async fn quasimodo_error() {}

    #[tokio::test]
    async fn quasimodo_hard_error() {}

    #[tokio::test]
    async fn quasimodo_no_liquidity() {}

    #[tokio::test]
    async fn quasimodo_infeasible() {}

    #[tokio::test]
    async fn same_token() {}

    #[tokio::test]
    async fn unsupported_token() {}

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let quasimodo_url =
            std::env::var("QUASIMODO_URL").expect("env variable QUASIMODO_URL is required");
        let infura_project_id =
            std::env::var("INFURA_PROJECT_ID").expect("env variable INFURA_PROJECT_ID is required");

        let t1 = ("WETH", testlib::tokens::WETH);
        let t2 = ("USDC", testlib::tokens::USDC);
        let amount1: U256 = U256::from(1) * U256::exp10(18);
        let amount2: U256 = U256::from(1) * U256::exp10(9);

        let client = Client::new();

        let transport = HttpTransport::new(
            client.clone(),
            Url::parse("https://mainnet.infura.io/v3/")
                .unwrap()
                .join(&infura_project_id)
                .unwrap(),
            "main".into(),
        );
        let web3 = Web3::new(DynTransport::new(transport));
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();

        let pools = Arc::new(
            PoolCache::new(
                CacheConfig::default(),
                uniswap_v2::get_liquidity_source(&web3).await.unwrap().1,
                current_block_stream(web3.clone(), Duration::from_secs(1))
                    .await
                    .unwrap(),
                Arc::new(NoopPoolCacheMetrics),
            )
            .unwrap(),
        );
        let token_info = Arc::new(TokenInfoFetcher { web3: web3.clone() });
        let contracts = BalancerContracts::new(&web3).await.unwrap();
        let current_block_stream = current_block_stream(web3.clone(), Duration::from_secs(10))
            .await
            .unwrap();
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                token_info.clone(),
                BalancerFactoryKind::value_variants(),
                Default::default(),
                current_block_stream.clone(),
                Arc::new(crate::sources::balancer_v2::pool_fetching::NoopBalancerPoolCacheMetrics),
                client.clone(),
                &contracts,
            )
            .await
            .expect("failed to create Balancer pool fetcher"),
        );
        let gas_info = Arc::new(web3);

        let estimator = QuasimodoPriceEstimator {
            api: Arc::new(DefaultHttpSolverApi {
                name: "test",
                network_name: "1".to_string(),
                chain_id: 1,
                base: Url::parse(&quasimodo_url).expect("failed to parse quasimodo url"),
                client,
                config: SolverConfig {
                    api_key: None,
                    max_nr_exec_orders: 100,
                    has_ucp_policy_parameter: false,
                    use_internal_buffers: true.into(),
                },
            }),
            sharing: Default::default(),
            pools,
            balancer_pools: Some(balancer_pool_fetcher),
            token_info,
            gas_info,
            native_token: testlib::tokens::WETH,
            base_tokens: Arc::new(BaseTokens::new(
                testlib::tokens::WETH,
                &[testlib::tokens::WETH, t1.1, t2.1],
            )),
        };

        let result = estimator
            .estimate(&Query {
                sell_token: t1.1,
                buy_token: t2.1,
                in_amount: amount1,
                kind: OrderKind::Sell,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "{} {} buys {} {}, costing {} gas",
            amount1.to_f64_lossy() / 1e18,
            t1.0,
            estimate.out_amount.to_f64_lossy() / 1e6,
            t2.0,
            estimate.gas,
        );

        let result = estimator
            .estimate(&Query {
                sell_token: t1.1,
                buy_token: t2.1,
                in_amount: amount2,
                kind: OrderKind::Buy,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "{} {} costs {} {}, costing {} gas",
            amount2.to_f64_lossy() / 1e6,
            t2.0,
            estimate.out_amount.to_f64_lossy() / 1e18,
            t1.0,
            estimate.gas,
        );
    }
}
