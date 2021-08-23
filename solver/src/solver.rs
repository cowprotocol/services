use crate::{liquidity::Liquidity, settlement::Settlement};
use anyhow::Result;
use baseline_solver::BaselineSolver;
use contracts::GPv2Settlement;
use ethcontract::{Account, H160, U256};
use http_solver::{buffers::BufferRetriever, HttpSolver, SolverConfig};
use matcha_solver::MatchaSolver;
use naive_solver::NaiveSolver;
use oneinch_solver::OneInchSolver;
use paraswap_solver::ParaswapSolver;
use reqwest::{Client, Url};
use shared::{
    conversions::U256Ext, price_estimate::PriceEstimating, token_info::TokenInfoFetching, Web3,
};
use single_order_solver::SingleOrderSolver;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};
use structopt::clap::arg_enum;

mod baseline_solver;
mod http_solver;
mod matcha_solver;
mod naive_solver;
mod oneinch_solver;
mod paraswap_solver;
mod single_order_solver;
mod solver_utils;

/// Interface that all solvers must implement.
///
/// A `solve` method transforming a collection of `Liquidity` (sources) into a list of
/// independent `Settlements`. Solvers are free to choose which types `Liquidity` they
/// would like to process, including their own private sources.
#[async_trait::async_trait]
pub trait Solver: 'static {
    /// Runs the solver.
    ///
    /// The returned settlements should be independent (for example not reusing the same user
    /// order) so that they can be merged by the driver at its leisure.
    async fn solve(
        &self,
        orders: Vec<Liquidity>,
        gas_price: f64,
        deadline: Instant,
    ) -> Result<Vec<Settlement>>;

    /// Returns solver's account that should be used to submit settlements.
    fn account(&self) -> &Account;

    /// Returns displayable name of the solver.
    ///
    /// This method is used for logging and metrics collection.
    fn name(&self) -> &'static str;
}

/// A vector of solvers.
pub type Solvers = Vec<Arc<dyn Solver>>;

/// A single settlement and a solver that've produced it.
pub type SettlementWithSolver = (Arc<dyn Solver>, Settlement);

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        Baseline,
        Mip,
        OneInch,
        Paraswap,
        Matcha,
        Quasimodo,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    account: Account,
    web3: Web3,
    solvers: Vec<SolverType>,
    base_tokens: HashSet<H160>,
    native_token: H160,
    mip_solver_url: Url,
    quasimodo_solver_url: Url,
    settlement_contract: &GPv2Settlement,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
    network_id: String,
    chain_id: u64,
    fee_subsidy_factor: f64,
    min_order_size_one_inch: U256,
    disabled_one_inch_protocols: Vec<String>,
    paraswap_slippage_bps: usize,
    disabled_paraswap_dexs: Vec<String>,
    client: Client,
) -> Result<Solvers> {
    // Tiny helper function to help out with type inference. Otherwise, all
    // `Box::new(...)` expressions would have to be cast `as Box<dyn Solver>`.
    #[allow(clippy::unnecessary_wraps)]
    fn shared(solver: impl Solver + 'static) -> Result<Arc<dyn Solver>> {
        Ok(Arc::new(solver))
    }

    let buffer_retriever = Arc::new(BufferRetriever::new(
        web3.clone(),
        settlement_contract.address(),
    ));
    // Helper function to create http solver instances.
    let create_http_solver = |url: Url, name: &'static str| -> HttpSolver {
        HttpSolver::new(
            name,
            account.clone(),
            url,
            None,
            SolverConfig {
                max_nr_exec_orders: 100,
            },
            native_token,
            token_info_fetcher.clone(),
            price_estimator.clone(),
            buffer_retriever.clone(),
            network_id.clone(),
            chain_id,
            fee_subsidy_factor,
            client.clone(),
        )
    };

    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => shared(NaiveSolver::new(account.clone())),
            SolverType::Baseline => {
                shared(BaselineSolver::new(account.clone(), base_tokens.clone()))
            }
            SolverType::Mip => shared(create_http_solver(mip_solver_url.clone(), "Mip")),
            SolverType::Quasimodo => shared(create_http_solver(
                quasimodo_solver_url.clone(),
                "Quasimodo",
            )),
            SolverType::OneInch => {
                let one_inch_solver: SingleOrderSolver<_> = OneInchSolver::with_disabled_protocols(
                    account.clone(),
                    web3.clone(),
                    settlement_contract.clone(),
                    chain_id,
                    disabled_one_inch_protocols.clone(),
                    client.clone(),
                )?
                .into();
                // We only want to use 1Inch for high value orders
                shared(SellVolumeFilteringSolver::new(
                    Box::new(one_inch_solver),
                    price_estimator.clone(),
                    native_token,
                    min_order_size_one_inch,
                ))
            }
            SolverType::Matcha => {
                let matcha_solver = MatchaSolver::new(
                    account.clone(),
                    web3.clone(),
                    settlement_contract.clone(),
                    chain_id,
                    client.clone(),
                )
                .unwrap();
                shared(SingleOrderSolver::from(matcha_solver))
            }
            SolverType::Paraswap => shared(SingleOrderSolver::from(ParaswapSolver::new(
                account.clone(),
                web3.clone(),
                settlement_contract.clone(),
                account.address(),
                token_info_fetcher.clone(),
                paraswap_slippage_bps,
                disabled_paraswap_dexs.clone(),
                client.clone(),
            ))),
        })
        .collect()
}

/// Returns a naive solver to be used e.g. in e2e tests.
pub fn naive_solver(account: Account) -> Arc<dyn Solver> {
    Arc::new(NaiveSolver::new(account))
}

/// A solver that remove limit order below a certain threshold and
/// passes the remaining liquidity onto an inner solver implementation.
pub struct SellVolumeFilteringSolver {
    inner: Box<dyn Solver + Send + Sync>,
    price_estimator: Arc<dyn PriceEstimating>,
    denominator_token: H160,
    min_value: U256,
}

impl SellVolumeFilteringSolver {
    pub fn new(
        inner: Box<dyn Solver + Send + Sync>,
        price_estimator: Arc<dyn PriceEstimating>,
        denominator_token: H160,
        min_value: U256,
    ) -> Self {
        Self {
            inner,
            price_estimator,
            denominator_token,
            min_value,
        }
    }

    async fn filter_liquidity(&self, orders: Vec<Liquidity>) -> Vec<Liquidity> {
        let sell_tokens: Vec<_> = orders
            .iter()
            .filter_map(|order| {
                if let Liquidity::Limit(order) = order {
                    Some(order.sell_token)
                } else {
                    None
                }
            })
            .collect();
        let prices: HashMap<_, _> = self
            .price_estimator
            .estimate_prices(&sell_tokens, self.denominator_token)
            .await
            .into_iter()
            .zip(sell_tokens)
            .filter_map(|(result, token)| {
                if let Ok(price) = result {
                    Some((token, price))
                } else {
                    None
                }
            })
            .collect();

        orders
            .into_iter()
            .filter(|order| {
                if let Liquidity::Limit(order) = order {
                    prices
                        .get(&order.sell_token)
                        .map(|price| {
                            price * order.sell_amount.to_big_rational()
                                > self.min_value.to_big_rational()
                        })
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl Solver for SellVolumeFilteringSolver {
    async fn solve(
        &self,
        orders: Vec<Liquidity>,
        gas_price: f64,
        deadline: Instant,
    ) -> Result<Vec<Settlement>> {
        let original_length = orders.len();
        let filtered_liquidity = self.filter_liquidity(orders).await;
        tracing::info!(
            "Filtered {} orders because on insufficient volume",
            original_length - filtered_liquidity.len()
        );
        self.inner
            .solve(filtered_liquidity, gas_price, deadline)
            .await
    }

    fn account(&self) -> &Account {
        self.inner.account()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }
}

#[cfg(test)]
mod tests {
    use num::BigRational;
    use shared::price_estimate::mocks::{FailingPriceEstimator, FakePriceEstimator};

    use crate::liquidity::LimitOrder;

    use super::*;

    /// Dummy solver returning no settlements
    pub struct NoopSolver();
    #[async_trait::async_trait]
    impl Solver for NoopSolver {
        async fn solve(&self, _: Vec<Liquidity>, _: f64, _: Instant) -> Result<Vec<Settlement>> {
            Ok(Vec::new())
        }

        fn account(&self) -> &Account {
            unimplemented!()
        }

        fn name(&self) -> &'static str {
            "NoopSolver"
        }
    }

    #[tokio::test]
    async fn test_filtering_solver_removes_limit_orders_with_too_little_volume() {
        let sell_token = H160::from_low_u64_be(1);
        let liquidity = vec![
            // Only filter limit orders
            Liquidity::ConstantProduct(Default::default()),
            // Orders with high enough amount
            Liquidity::Limit(LimitOrder {
                sell_amount: 100_000.into(),
                sell_token,
                ..Default::default()
            }),
            Liquidity::Limit(LimitOrder {
                sell_amount: 500_000.into(),
                sell_token,
                ..Default::default()
            }),
            // Order with small amount
            Liquidity::Limit(LimitOrder {
                sell_amount: 100.into(),
                sell_token,
                ..Default::default()
            }),
        ];

        let price_estimator = Arc::new(FakePriceEstimator(BigRational::from_integer(42.into())));
        let solver = SellVolumeFilteringSolver {
            inner: Box::new(NoopSolver()),
            price_estimator,
            denominator_token: H160::zero(),
            min_value: 400_000.into(),
        };
        assert_eq!(solver.filter_liquidity(liquidity).await.len(), 3);
    }

    #[tokio::test]
    async fn test_filtering_solver_removes_orders_without_price_estimate() {
        let sell_token = H160::from_low_u64_be(1);
        let liquidity = vec![Liquidity::Limit(LimitOrder {
            sell_amount: 100_000.into(),
            sell_token,
            ..Default::default()
        })];

        let price_estimator = Arc::new(FailingPriceEstimator());
        let solver = SellVolumeFilteringSolver {
            inner: Box::new(NoopSolver()),
            price_estimator,
            denominator_token: H160::zero(),
            min_value: 0.into(),
        };
        assert_eq!(solver.filter_liquidity(liquidity).await.len(), 0);
    }
}
