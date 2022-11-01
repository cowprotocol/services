use crate::{
    baseline_solver::BaseTokens,
    http_solver::{
        gas_model::GasModel,
        model::{
            AmmModel, AmmParameters, BatchAuctionModel, ConcentratedPoolParameters,
            ConstantProductPoolParameters, MetadataModel, OrderModel, SettledBatchAuctionModel,
            StablePoolParameters, TokenAmount, TokenInfoModel, WeightedPoolTokenData,
            WeightedProductPoolParameters,
        },
        HttpSolverApi,
    },
    price_estimation::{
        gas::{ERC20_TRANSFER, GAS_PER_ORDER, INITIALIZATION_COST, SETTLEMENT},
        rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
    rate_limiter::RateLimiter,
    recent_block_cache::Block,
    request_sharing::RequestSharing,
    sources::{
        balancer_v2::{pools::common::compute_scaling_rate, BalancerPoolFetching},
        uniswap_v2::pool_fetching::PoolFetching as UniswapV2PoolFetching,
        uniswap_v3::pool_fetching::PoolFetching as UniswapV3PoolFetching,
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

pub struct HttpPriceEstimator {
    api: Arc<dyn HttpSolverApi>,
    sharing: RequestSharing<
        Query,
        BoxFuture<'static, Result<SettledBatchAuctionModel, PriceEstimationError>>,
    >,
    pools: Arc<dyn UniswapV2PoolFetching>,
    balancer_pools: Option<Arc<dyn BalancerPoolFetching>>,
    uniswap_v3_pools: Option<Arc<dyn UniswapV3PoolFetching>>,
    token_info: Arc<dyn TokenInfoFetching>,
    gas_info: Arc<dyn GasPriceEstimating>,
    native_token: H160,
    base_tokens: Arc<BaseTokens>,
    network_name: String,
    rate_limiter: Arc<RateLimiter>,
}

impl HttpPriceEstimator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api: Arc<dyn HttpSolverApi>,
        pools: Arc<dyn UniswapV2PoolFetching>,
        balancer_pools: Option<Arc<dyn BalancerPoolFetching>>,
        uniswap_v3_pools: Option<Arc<dyn UniswapV3PoolFetching>>,
        token_info: Arc<dyn TokenInfoFetching>,
        gas_info: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        base_tokens: Arc<BaseTokens>,
        network_name: String,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            api,
            sharing: Default::default(),
            pools,
            balancer_pools,
            uniswap_v3_pools,
            token_info,
            gas_info,
            native_token,
            base_tokens,
            network_name,
            rate_limiter,
        }
    }

    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        let gas_price = U256::from_f64_lossy(self.gas_info.estimate().await?.effective_gas_price())
            .max(1.into()); // flooring at 1 to avoid division by zero error

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (U256::max_value(), query.in_amount),
            OrderKind::Sell => (query.in_amount, U256::one()),
        };

        let orders = maplit::btreemap! {
            Default::default() => OrderModel {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                sell_amount,
                buy_amount,
                allow_partial_fill: false,
                is_sell_order: query.kind == OrderKind::Sell,
                fee: TokenAmount {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                cost: TokenAmount {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                is_liquidity_order: false,
                mandatory: true,
                has_atomic_execution: false,
                // TODO: is it possible to set a more accurate reward?
                reward: 35.,
            },
        };

        let token_pair = TokenPair::new(query.sell_token, query.buy_token).unwrap();
        let pairs = self.base_tokens.relevant_pairs([token_pair].into_iter());
        let gas_model = GasModel {
            native_token: self.native_token,
            gas_price: gas_price.to_f64_lossy(),
        };

        let (uniswap_pools, balancer_pools, uniswap_v3_pools) = futures::try_join!(
            self.uniswap_pools(pairs.clone(), &gas_model),
            self.balancer_pools(pairs.clone(), &gas_model),
            self.uniswap_v3_pools(pairs.clone(), &gas_model)
        )?;
        let amms: BTreeMap<usize, AmmModel> = uniswap_pools
            .into_iter()
            .chain(balancer_pools)
            .chain(uniswap_v3_pools)
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
                AmmParameters::Concentrated(params) => {
                    tokens.extend(params.pool.tokens.iter().map(|token| token.id))
                }
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
            metadata: Some(MetadataModel {
                environment: Some(self.network_name.clone()),
                gas_price: Some(gas_price.to_f64_lossy()),
                native_token: Some(self.native_token),
                ..Default::default()
            }),
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
        let settlement_future = rate_limited(self.rate_limiter.clone(), settlement_future);
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
        for interaction in settlement.interaction_data {
            cost += self.extract_cost(&interaction.cost)?;
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
                address: pool.address,
            })
            .collect())
    }

    async fn uniswap_v3_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = match &self.uniswap_v3_pools {
            Some(uniswap_v3) => uniswap_v3
                .fetch(&pairs, Block::Recent)
                .await
                .context("no uniswap v3 pools")?,
            None => return Ok(Default::default()),
        };
        Ok(pools
            .into_iter()
            .map(|pool| AmmModel {
                fee: BigRational::from((
                    BigInt::from(*pool.state.fee.numer()),
                    BigInt::from(*pool.state.fee.denom()),
                )),
                cost: gas_model.cost_for_gas(pool.gas_stats.mean_gas),
                address: pool.address,
                parameters: AmmParameters::Concentrated(ConcentratedPoolParameters { pool }),
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
            address: pool.common.address,
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
                    address: pool.common.address,
                })
            });
        let mut models = Vec::from_iter(weighted);
        for stable in stable {
            models.push(stable?);
        }
        Ok(models)
    }

    fn extract_cost(&self, cost: &Option<TokenAmount>) -> Result<U256, PriceEstimationError> {
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

impl PriceEstimating for HttpPriceEstimator {
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
    use crate::{
        current_block::current_block_stream,
        gas_price_estimation::FakeGasPriceEstimator,
        http_solver::{
            model::{ExecutedAmmModel, ExecutedOrderModel, InteractionData, UpdatedAmmModel},
            DefaultHttpSolverApi, MockHttpSolverApi, SolverConfig,
        },
        price_estimation::Query,
        recent_block_cache::CacheConfig,
        sources::{
            balancer_v2::{
                pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher,
            },
            uniswap_v2,
            uniswap_v2::{pool_cache::PoolCache, pool_fetching::test_util::FakePoolFetcher},
            uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
        },
        token_info::{MockTokenInfoFetching, TokenInfoFetcher},
        transport::http::HttpTransport,
        Web3,
    };
    use anyhow::bail;
    use clap::ValueEnum;
    use ethcontract::dyns::DynTransport;
    use gas_estimation::GasPrice1559;
    use maplit::hashmap;
    use model::order::OrderKind;
    use reqwest::Client;
    use std::collections::HashMap;
    use url::Url;

    #[tokio::test]
    async fn test_estimate() {
        let native_token = H160::zero();
        let mut api = MockHttpSolverApi::new();
        api.expect_solve().returning(move |_, _| {
            Ok(SettledBatchAuctionModel {
                orders: hashmap! {
                    0 => ExecutedOrderModel {
                        exec_sell_amount: 50.into(),
                        exec_buy_amount: 200.into(),
                        cost: None,
                        fee: None,
                        exec_plan: None,
                    }
                },
                ..Default::default()
            })
        });

        let mut token_info_fetching = MockTokenInfoFetching::new();
        token_info_fetching
            .expect_get_token_infos()
            .returning(move |_| HashMap::new());

        let gas_price_estimating = Arc::new(FakeGasPriceEstimator::new(GasPrice1559::default()));

        let estimator = HttpPriceEstimator::new(
            Arc::new(api),
            Arc::new(FakePoolFetcher(vec![])),
            None,
            None,
            Arc::new(token_info_fetching),
            gas_price_estimating,
            native_token,
            Arc::new(BaseTokens::new(native_token, &[])),
            "test".into(),
            RateLimiter::test(),
        );

        let sell_order = estimator
            .estimate(&Query {
                from: None,
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                in_amount: 100.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();
        assert_eq!(sell_order.out_amount, 200.into());

        let buy_order = estimator
            .estimate(&Query {
                from: None,
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                in_amount: 100.into(),
                kind: OrderKind::Buy,
            })
            .await
            .unwrap();
        assert_eq!(buy_order.out_amount, 50.into());
    }

    #[tokio::test]
    async fn test_api_error() {
        let native_token = H160::zero();
        let mut api = MockHttpSolverApi::new();
        api.expect_solve()
            .returning(move |_, _| bail!("solver error"));

        let mut token_info_fetching = MockTokenInfoFetching::new();
        token_info_fetching
            .expect_get_token_infos()
            .returning(move |_| HashMap::new());

        let gas_price_estimating = Arc::new(FakeGasPriceEstimator::new(GasPrice1559::default()));

        let estimator = HttpPriceEstimator::new(
            Arc::new(api),
            Arc::new(FakePoolFetcher(vec![])),
            None,
            None,
            Arc::new(token_info_fetching),
            gas_price_estimating,
            native_token,
            Arc::new(BaseTokens::new(native_token, &[])),
            "test".into(),
            RateLimiter::test(),
        );
        let err = estimator
            .estimate(&Query {
                from: None,
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                in_amount: 100.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, PriceEstimationError::Other(_)));
    }

    #[tokio::test]
    async fn test_no_liquidity() {
        let native_token = H160::zero();
        let mut api = MockHttpSolverApi::new();
        api.expect_solve().returning(move |_, _| {
            Ok(SettledBatchAuctionModel {
                orders: HashMap::new(), // no matched order
                ..Default::default()
            })
        });

        let mut token_info_fetching = MockTokenInfoFetching::new();
        token_info_fetching
            .expect_get_token_infos()
            .returning(move |_| HashMap::new());

        let gas_price_estimating = Arc::new(FakeGasPriceEstimator::new(GasPrice1559::default()));

        let estimator = HttpPriceEstimator::new(
            Arc::new(api),
            Arc::new(FakePoolFetcher(vec![])),
            None,
            None,
            Arc::new(token_info_fetching),
            gas_price_estimating,
            native_token,
            Arc::new(BaseTokens::new(native_token, &[])),
            "test".into(),
            RateLimiter::test(),
        );

        let err = estimator
            .estimate(&Query {
                from: None,
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                in_amount: 100.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, PriceEstimationError::NoLiquidity));
    }

    #[tokio::test]
    async fn test_gas_estimate() {
        let native_token = H160::zero();
        let mut api = MockHttpSolverApi::new();
        api.expect_solve().returning(move |_, _| {
            Ok(SettledBatchAuctionModel {
                orders: hashmap! {
                    0 => ExecutedOrderModel {
                        exec_sell_amount: 100.into(),
                        exec_buy_amount: 100.into(),
                        cost: Some(TokenAmount {
                            amount: 100_000.into(),
                            token: native_token
                        }),
                        fee: None,
                        exec_plan: None,
                    }
                },
                amms: hashmap! {
                    0 => UpdatedAmmModel {
                        execution: vec![ExecutedAmmModel {
                            sell_token: H160::from_low_u64_be(0),
                            buy_token: H160::from_low_u64_be(1),
                            exec_sell_amount: 100.into(),
                            exec_buy_amount: 100.into(),
                            exec_plan: None
                        },ExecutedAmmModel {
                            sell_token: H160::from_low_u64_be(1),
                            buy_token: H160::from_low_u64_be(0),
                            exec_sell_amount: 100.into(),
                            exec_buy_amount: 100.into(),
                            exec_plan: None
                        }],
                        cost: Some(TokenAmount {
                            amount: 200_000.into(),
                            token: native_token
                        }
                        ),
                    }
                },
                interaction_data: vec![InteractionData {
                    target: H160::zero(),
                    value: U256::zero(),
                    call_data: vec![],
                    inputs: vec![],
                    outputs: vec![],
                    exec_plan: None,
                    cost: Some(TokenAmount {
                        amount: 300_000.into(),
                        token: native_token,
                    }),
                }],
                ..Default::default()
            })
        });

        let mut token_info_fetching = MockTokenInfoFetching::new();
        token_info_fetching
            .expect_get_token_infos()
            .returning(move |_| HashMap::new());

        let gas_price_estimating = Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
            max_fee_per_gas: 1.0,
            max_priority_fee_per_gas: 1.0,
            ..Default::default()
        }));

        let estimator = HttpPriceEstimator::new(
            Arc::new(api),
            Arc::new(FakePoolFetcher(vec![])),
            None,
            None,
            Arc::new(token_info_fetching),
            gas_price_estimating,
            native_token,
            Arc::new(BaseTokens::new(native_token, &[])),
            "test".into(),
            RateLimiter::test(),
        );

        let query = Query {
            from: None,
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            in_amount: 100.into(),
            kind: OrderKind::Sell,
        };
        let result = estimator.estimate(&query).await.unwrap();

        // 94391 base cost + 100k order cost + 200k AMM cost (x2) + 300k interaction cost
        assert_eq!(result.gas, 894391);
    }

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
            )
            .unwrap(),
        );
        let token_info = Arc::new(TokenInfoFetcher { web3: web3.clone() });
        let contracts =
            BalancerContracts::new(&web3, BalancerFactoryKind::value_variants().to_vec())
                .await
                .unwrap();
        let current_block_stream = current_block_stream(web3.clone(), Duration::from_secs(10))
            .await
            .unwrap();
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                token_info.clone(),
                Default::default(),
                current_block_stream.clone(),
                client.clone(),
                web3.clone(),
                &contracts,
                Default::default(),
            )
            .await
            .expect("failed to create Balancer pool fetcher"),
        );
        let uniswap_v3_pool_fetcher = Arc::new(
            UniswapV3PoolFetcher::new(chain_id, client.clone(), web3.clone(), 100)
                .await
                .expect("failed to create uniswap v3 pool fetcher"),
        );
        let gas_info = Arc::new(web3);

        let estimator = HttpPriceEstimator {
            api: Arc::new(DefaultHttpSolverApi {
                name: "test".to_string(),
                network_name: "1".to_string(),
                chain_id: 1,
                base: Url::parse(&quasimodo_url).expect("failed to parse quasimodo url"),
                client,
                config: SolverConfig {
                    use_internal_buffers: Some(true),
                    ..Default::default()
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
            network_name: "Ethereum / Mainnet".to_string(),
            rate_limiter: Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
            uniswap_v3_pools: Some(uniswap_v3_pool_fetcher),
        };

        let result = estimator
            .estimate(&Query {
                from: None,
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
                from: None,
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
