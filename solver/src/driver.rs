use crate::chain;
use crate::{
    liquidity::{uniswap::UniswapLiquidity, LimitOrder, Liquidity},
    orderbook::OrderBookApi,
    settlement::Settlement,
    settlement_submission,
    solver::Solver,
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use futures::future::join_all;
use gas_estimation::GasPriceEstimating;
use itertools::Itertools;
use num::BigRational;
use primitive_types::H160;
use shared::price_estimate::PriceEstimating;
use std::collections::HashMap;
use std::{cmp::Reverse, sync::Arc, time::Duration};
use tracing::info;

// There is no economic viability calculation yet so we're using an arbitrary very high cap to
// protect against a gas estimator giving bogus results that would drain all our funds.
const GAS_PRICE_CAP: f64 = 500e9;

pub struct Driver {
    settlement_contract: GPv2Settlement,
    orderbook: OrderBookApi,
    uniswap_liquidity: UniswapLiquidity,
    price_estimator: Arc<dyn PriceEstimating>,
    solver: Vec<Box<dyn Solver>>,
    gas_price_estimator: Box<dyn GasPriceEstimating>,
    target_confirm_time: Duration,
    settle_interval: Duration,
    native_token: H160,
}

impl Driver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_contract: GPv2Settlement,
        uniswap_liquidity: UniswapLiquidity,
        orderbook: OrderBookApi,
        price_estimator: Arc<dyn PriceEstimating>,
        solver: Vec<Box<dyn Solver>>,
        gas_price_estimator: Box<dyn GasPriceEstimating>,
        target_confirm_time: Duration,
        settle_interval: Duration,
        native_token: H160,
    ) -> Self {
        Self {
            settlement_contract,
            orderbook,
            uniswap_liquidity,
            price_estimator,
            solver,
            gas_price_estimator,
            target_confirm_time,
            settle_interval,
            native_token,
        }
    }

    pub async fn run_forever(&mut self) -> ! {
        loop {
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run finished ok"),
                Err(err) => tracing::error!("single run errored: {:?}", err),
            }
            tokio::time::delay_for(self.settle_interval).await;
        }
    }

    async fn collect_estimated_prices(
        &self,
        limit_orders: &[LimitOrder],
    ) -> HashMap<H160, BigRational> {
        // Computes set of traded tokens (limit orders only).
        let tokens: Vec<H160> = limit_orders
            .iter()
            // .flat_map(|lo| vec![lo.sell_token, lo.buy_token].into_iter())
            .flat_map(|lo| chain![lo.sell_token, lo.buy_token])
            .sorted()
            .dedup()
            .collect();

        // For ranking purposes it doesn't matter how the external price vector is scaled,
        // but native_token is used here anyway for better logging/debugging.
        let denominator_token: H160 = self.native_token;

        let estimated_prices = self
            .price_estimator
            .estimate_prices(tokens.as_slice(), denominator_token)
            .await;

        tokens
            .into_iter()
            .zip(estimated_prices)
            .filter_map(|(token, price)| match price {
                Ok(price) => Some((token, price)),
                Err(err) => {
                    tracing::warn!("failed to estimate price for token {}: {:?}", token, err);
                    None
                }
            })
            .collect()
    }

    pub async fn single_run(&mut self) -> Result<()> {
        tracing::debug!("starting single run");
        let mut limit_orders = self
            .orderbook
            .get_liquidity()
            .await
            .context("failed to get orderbook")?;
        tracing::debug!("got {} orders", limit_orders.len());

        let estimated_prices = self.collect_estimated_prices(&limit_orders).await;
        let original_length = limit_orders.len();
        limit_orders.retain(|lo| {
            [lo.sell_token, lo.buy_token]
                .iter()
                .all(|token| estimated_prices.contains_key(token))
        });
        let removed_orders = original_length - limit_orders.len();
        if removed_orders > 0 {
            tracing::debug!("pruned {} orders", removed_orders);
        }

        let amms = self
            .uniswap_liquidity
            .get_liquidity(limit_orders.iter())
            .await
            .context("failed to get uniswap pools")?;
        tracing::debug!("got {} AMMs", amms.len());

        let liquidity: Vec<Liquidity> = limit_orders
            .into_iter()
            .map(Liquidity::Limit)
            .chain(amms.into_iter().map(Liquidity::Amm))
            .collect();

        let mut settlements: Vec<(&Box<dyn Solver>, Settlement)> =
            join_all(self.solver.iter().map(|solver| {
                let liquidity = liquidity.clone();
                async move { (solver, solver.solve(liquidity).await) }
            }))
            .await
            .into_iter()
            .filter_map(|(solver, settlement)| match settlement {
                Ok(settlement) => settlement.map(|settlement| (solver, settlement)),
                Err(err) => {
                    tracing::error!("solver {} error: {:?}", solver, err);
                    None
                }
            })
            .collect();
        for (solver, settlement) in settlements.iter() {
            info!(
                "{} found solution with objective value: {}",
                solver,
                settlement.objective_value(&estimated_prices)
            );
        }

        // Sort by key in descending order
        settlements.sort_by_cached_key(|(_, settlement)| {
            Reverse(settlement.objective_value(&estimated_prices))
        });
        for (solver, settlement) in settlements {
            info!("{} computed {:?}", solver, settlement);
            if settlement.trades.is_empty() {
                info!("Skipping empty settlement");
                continue;
            }
            match settlement_submission::submit(
                &self.settlement_contract,
                self.gas_price_estimator.as_ref(),
                self.target_confirm_time,
                GAS_PRICE_CAP,
                settlement,
            )
            .await
            {
                Ok(_) => {
                    // TODO: order validity checks
                    // Decide what is handled by orderbook service and what by us.
                    // We likely want to at least mark orders we know we have settled so that we don't
                    // attempt to settle them again when they are still in the orderbook.
                    break;
                }
                Err(err) => tracing::error!("{} Failed to submit settlement: {:?}", solver, err),
            }
        }
        Ok(())
    }
}
