use crate::bad_token::BadTokenDetecting;
use crate::baseline_solver::BaseTokens;
use crate::http_solver_api::model::{
    AmmModel, AmmParameters, BatchAuctionModel, ConstantProductPoolParameters, CostModel, FeeModel,
    OrderModel, TokenInfoModel,
};
use crate::http_solver_api::HttpSolverApi;
use crate::price_estimation::gas::{
    ERC20_TRANSFER, GAS_PER_ORDER, GAS_PER_UNISWAP, INITIALIZATION_COST, SETTLEMENT,
};
use crate::price_estimation::{
    ensure_token_supported, Estimate, PriceEstimating, PriceEstimationError, Query,
};
use crate::recent_block_cache::Block;
use crate::sources::uniswap_v2::pool_cache::PoolCache;
use crate::sources::uniswap_v2::pool_fetching::PoolFetching;
use crate::token_info::TokenInfoFetching;
use ethcontract::{H160, U256};
use gas_estimation::GasPriceEstimating;
use model::order::OrderKind;
use model::TokenPair;
use num::{BigInt, BigRational};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct QuasimodoPriceEstimator {
    pub api: Arc<HttpSolverApi>,
    pub pools: Arc<PoolCache>,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
    pub token_info: Arc<dyn TokenInfoFetching>,
    pub gas_info: Arc<dyn GasPriceEstimating>,
    pub native_token: H160,
    pub base_tokens: Arc<BaseTokens>,
}

impl QuasimodoPriceEstimator {
    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        if query.buy_token == query.sell_token {
            return Ok(Estimate {
                out_amount: query.in_amount,
                gas: 0.into(),
            });
        }

        ensure_token_supported(query.buy_token, self.bad_token_detector.as_ref()).await?;
        ensure_token_supported(query.sell_token, self.bad_token_detector.as_ref()).await?;

        let gas_price = U256::from_f64_lossy(self.gas_info.estimate().await?.effective_gas_price());

        let mut tokens = self.base_tokens.tokens().clone();
        tokens.insert(query.sell_token);
        tokens.insert(query.buy_token);
        tokens.insert(self.native_token);
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
            },
        };

        let token_pair = TokenPair::new(query.sell_token, query.buy_token).unwrap();
        let pairs = self.base_tokens.relevant_pairs([token_pair].into_iter());

        let amms = self
            .pools
            .fetch(pairs, Block::Recent)
            .await?
            .iter()
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
                cost: CostModel {
                    amount: U256::from(GAS_PER_UNISWAP) * gas_price,
                    token: self.native_token,
                },
                mandatory: false,
            })
            .enumerate()
            .collect();

        let settlement = self
            .api
            .solve(
                &BatchAuctionModel {
                    tokens,
                    orders,
                    amms,
                    metadata: None,
                },
                Instant::now() + Duration::from_secs(1),
            )
            .await?;

        if settlement.orders.is_empty() {
            return Err(PriceEstimationError::NoLiquidity);
        }

        let mut cost = self.extract_cost(&settlement.orders[&0].cost)?;
        for amm in settlement.amms.values() {
            cost += self.extract_cost(&amm.cost)? * amm.execution.len();
        }
        let gas = (cost / gas_price)
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

#[async_trait::async_trait]
impl PriceEstimating for QuasimodoPriceEstimator {
    async fn estimates(
        &self,
        queries: &[Query],
    ) -> Vec<anyhow::Result<Estimate, PriceEstimationError>> {
        let mut results = Vec::with_capacity(queries.len());

        for query in queries {
            results.push(self.estimate(query).await);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::list_based::ListBasedDetector;
    use crate::current_block::current_block_stream;
    use crate::http_solver_api::SolverConfig;
    use crate::price_estimation::Query;
    use crate::recent_block_cache::CacheConfig;
    use crate::sources::uniswap_v2::pair_provider::UniswapPairProvider;
    use crate::sources::uniswap_v2::pool_cache::NoopPoolCacheMetrics;
    use crate::sources::uniswap_v2::pool_fetching::PoolFetcher;
    use crate::token_info::TokenInfoFetcher;
    use crate::transport::http::HttpTransport;
    use crate::Web3;
    use contracts::UniswapV2Factory;
    use ethcontract::dyns::DynTransport;
    use model::order::OrderKind;
    use reqwest::Client;
    use url::Url;

    // TODO: to imolement these tests, we'll need to make HTTP solver API mockable.

    #[tokio::test]
    async fn estimate_sell() {}

    #[tokio::test]
    async fn estimate_buy() {}

    #[tokio::test]
    async fn quasimodo_error() {}

    #[tokio::test]
    async fn quasimodo_no_liquidity() {}

    #[tokio::test]
    async fn same_token() {}

    #[tokio::test]
    async fn unsupported_token() {}

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let quasimodo_url =
            std::env::var("QUASIMODO_URL").expect("env variable QUASIMODO_URL is required");

        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

        let _weth = ("weth", weth);
        let _owl = ("owl", addr!("1A5F9352Af8aF974bFC03399e3767DF6370d82e4"));
        let _bal = ("bal", addr!("ba100000625a3754423978a60c9317c58a424e3D"));
        let _gno = ("gno", addr!("6810e776880c02933d47db1b9fc05908e5386b96"));
        let _dai = ("dai", addr!("6B175474E89094C44Da98b954EedeAC495271d0F"));

        let t1 = _weth;
        let t2 = _owl;
        let amount: U256 = U256::from(1) * U256::exp10(8);

        let client = Client::new();

        let transport = HttpTransport::new(
            client.clone(),
            Url::parse("https://mainnet.infura.io/v3/5a2827cf14c44719baa03f3e6ed22118").unwrap(),
            "main".into(),
        );
        let web3 = Web3::new(DynTransport::new(transport));

        let pools = Arc::new(
            PoolCache::new(
                CacheConfig::default(),
                Box::new(PoolFetcher {
                    pair_provider: Arc::new(UniswapPairProvider {
                        factory: UniswapV2Factory::deployed(&web3).await.unwrap(),
                        chain_id: 1,
                    }),
                    web3: web3.clone(),
                }),
                current_block_stream(web3.clone(), Duration::from_secs(1))
                    .await
                    .unwrap(),
                Arc::new(NoopPoolCacheMetrics),
            )
            .unwrap(),
        );
        let bad_token_detector = Arc::new(ListBasedDetector::deny_list(Vec::new()));
        let token_info = Arc::new(TokenInfoFetcher { web3: web3.clone() });
        let gas_info = Arc::new(web3);

        let estimator = QuasimodoPriceEstimator {
            api: Arc::new(HttpSolverApi {
                name: "test",
                network_name: "1".to_string(),
                chain_id: 1,
                base: Url::parse(&quasimodo_url).expect("failed to parse quasimodo url"),
                client,
                config: SolverConfig {
                    api_key: None,
                    max_nr_exec_orders: 100,
                    has_ucp_policy_parameter: false,
                },
            }),
            pools,
            bad_token_detector,
            token_info,
            gas_info,
            native_token: weth,
            base_tokens: Arc::new(BaseTokens::new(weth, &[weth, t1.1, t2.1])),
        };

        let result = estimator
            .estimate(&Query {
                sell_token: t1.1,
                buy_token: t2.1,
                in_amount: amount,
                kind: OrderKind::Sell,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "{} {} buys {} {}, costing {} gas",
            amount.to_f64_lossy() / 1e18,
            t1.0,
            estimate.out_amount.to_f64_lossy() / 1e18,
            t2.0,
            estimate.gas,
        );

        let result = estimator
            .estimate(&Query {
                sell_token: t1.1,
                buy_token: t2.1,
                in_amount: amount,
                kind: OrderKind::Buy,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "{} {} costs {} {}, costing {} gas",
            amount.to_f64_lossy() / 1e18,
            t2.0,
            estimate.out_amount.to_f64_lossy() / 1e18,
            t1.0,
            estimate.gas,
        );
    }
}
