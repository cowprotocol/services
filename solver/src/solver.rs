use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use crate::{
    baseline_solver::BaselineSolver,
    http_solver::{HttpSolver, SolverConfig},
    liquidity::Liquidity,
    naive_solver::NaiveSolver,
    oneinch_solver::OneInchSolver,
    settlement::Settlement,
    single_order_solver::SingleOrderSolver,
};
use anyhow::Result;
use contracts::GPv2Settlement;
use ethcontract::{H160, U256};
use reqwest::Url;
use shared::{
    conversions::U256Ext, price_estimate::PriceEstimating, token_info::TokenInfoFetching,
};
use structopt::clap::arg_enum;

// For solvers that enforce a timeout internally we set their timeout to the global solver timeout
// minus this duration to account for additional delay for example from the network.
const TIMEOUT_SAFETY_BUFFER: Duration = Duration::from_secs(5);

/// Interface that all solvers must implement
/// A `solve` method transforming a collection of `Liquidity` (sources) into a list of
/// independent `Settlements`. Solvers are free to choose which types `Liquidity` they
/// would like to include/process (i.e. those already supported here or their own private sources)
/// The `name` method is included for logging purposes.
#[async_trait::async_trait]
pub trait Solver {
    // The returned settlements should be independent (for example not reusing the same user
    // order) so that they can be merged by the driver at its leisure.
    async fn solve(&self, orders: Vec<Liquidity>, gas_price: f64) -> Result<Vec<Settlement>>;

    // Displayable name of the solver. Defaults to the type name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

arg_enum! {
    #[derive(Debug)]
    pub enum SolverType {
        Naive,
        Baseline,
        Mip,
        OneInch,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    solvers: Vec<SolverType>,
    base_tokens: HashSet<H160>,
    native_token: H160,
    mip_solver_url: Url,
    settlement_contract: &GPv2Settlement,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    price_estimator: Arc<dyn PriceEstimating>,
    network_id: String,
    chain_id: u64,
    fee_discount_factor: f64,
    solver_timeout: Duration,
    min_order_size_one_inch: U256,
    disabled_one_inch_protocols: Vec<String>,
) -> Result<Vec<Box<dyn Solver>>> {
    // Tiny helper function to help out with type inference. Otherwise, all
    // `Box::new(...)` expressions would have to be cast `as Box<dyn Solver>`.
    #[allow(clippy::unnecessary_wraps)]
    fn boxed(solver: impl Solver + 'static) -> Result<Box<dyn Solver>> {
        Ok(Box::new(solver))
    }

    let time_limit = solver_timeout
        .checked_sub(TIMEOUT_SAFETY_BUFFER)
        .expect("solver_timeout too low");

    solvers
        .into_iter()
        .map(|solver_type| match solver_type {
            SolverType::Naive => boxed(NaiveSolver {}),
            SolverType::Baseline => boxed(BaselineSolver::new(base_tokens.clone())),
            SolverType::Mip => boxed(HttpSolver::new(
                mip_solver_url.clone(),
                None,
                SolverConfig {
                    max_nr_exec_orders: 100,
                    time_limit: time_limit.as_secs() as u32,
                },
                native_token,
                token_info_fetcher.clone(),
                price_estimator.clone(),
                network_id.clone(),
                chain_id,
                fee_discount_factor,
            )),
            SolverType::OneInch => {
                let one_inch_solver: SingleOrderSolver<OneInchSolver> =
                    OneInchSolver::with_disabled_protocols(
                        settlement_contract.clone(),
                        chain_id,
                        disabled_one_inch_protocols.clone(),
                    )?
                    .into();
                // We only want to use 1Inch for high value orders
                boxed(SellVolumeFilteringSolver::new(
                    Box::new(one_inch_solver),
                    price_estimator.clone(),
                    native_token,
                    min_order_size_one_inch,
                ))
            }
        })
        .collect()
}

/// Dummy solver returning no settlements
pub struct NoopSolver();
#[async_trait::async_trait]
impl Solver for NoopSolver {
    async fn solve(&self, _: Vec<Liquidity>, _: f64) -> Result<Vec<Settlement>> {
        Ok(Vec::new())
    }
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
    async fn solve(&self, orders: Vec<Liquidity>, gas_price: f64) -> Result<Vec<Settlement>> {
        let original_length = orders.len();
        let filtered_liquidity = self.filter_liquidity(orders).await;
        tracing::info!(
            "Filtered {} orders because on insufficient volume",
            original_length - filtered_liquidity.len()
        );
        self.inner.solve(filtered_liquidity, gas_price).await
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

    #[tokio::test]
    async fn test_filtering_solver_removes_limit_orders_with_too_little_volume() {
        let sell_token = H160::from_low_u64_be(1);
        let liquidity = vec![
            // Only filter limit orders
            Liquidity::Amm(Default::default()),
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
