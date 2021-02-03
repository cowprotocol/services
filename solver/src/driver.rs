use crate::{
    liquidity::{uniswap::UniswapLiquidity, Liquidity},
    orderbook::OrderBookApi,
    settlement_submission,
    solver::Solver,
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use gas_estimation::GasPriceEstimating;
use std::time::Duration;
use tracing::info;

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    settlement_contract: GPv2Settlement,
    orderbook: OrderBookApi,
    uniswap_liquidity: UniswapLiquidity,
    solver: Box<dyn Solver>,
    gas_price_estimator: Box<dyn GasPriceEstimating>,
    target_confirm_time: Duration,
}

impl Driver {
    pub fn new(
        settlement_contract: GPv2Settlement,
        uniswap_liquidity: UniswapLiquidity,
        orderbook: OrderBookApi,
        solver: Box<dyn Solver>,
        gas_price_estimator: Box<dyn GasPriceEstimating>,
        target_confirm_time: Duration,
    ) -> Self {
        Self {
            settlement_contract,
            orderbook,
            uniswap_liquidity,
            solver,
            gas_price_estimator,
            target_confirm_time,
        }
    }

    pub async fn run_forever(&mut self) -> ! {
        loop {
            match self.single_run().await {
                Ok(()) => tracing::debug!("single run finished ok"),
                Err(err) => tracing::error!("single run errored: {:?}", err),
            }
            tokio::time::delay_for(SETTLE_INTERVAL).await;
        }
    }

    pub async fn single_run(&mut self) -> Result<()> {
        tracing::debug!("starting single run");
        let limit_orders = self
            .orderbook
            .get_liquidity()
            .await
            .context("failed to get orderbook")?;
        tracing::debug!("got {} orders", limit_orders.len());

        let amms = self
            .uniswap_liquidity
            .get_liquidity(limit_orders.iter())
            .await
            .context("failed to get uniswap pools")?;

        let liquidity = limit_orders
            .into_iter()
            .map(Liquidity::Limit)
            .chain(amms.into_iter().map(Liquidity::Amm))
            .collect();

        // TODO: order validity checks
        // Decide what is handled by orderbook service and what by us.
        // We likely want to at least mark orders we know we have settled so that we don't
        // attempt to settle them again when they are still in the orderbook.
        let settlement = match self.solver.solve(liquidity).await? {
            None => return Ok(()),
            Some(settlement) => settlement,
        };
        info!("Computed {:?}", settlement);
        if settlement.trades.is_empty() {
            info!("Skipping empty settlement");
        }
        // TODO: check if we need to approve spending to uniswap
        settlement_submission::submit(
            settlement,
            &self.settlement_contract,
            self.gas_price_estimator.as_ref(),
            self.target_confirm_time,
        )
        .await
        .context("failed to submit settlement")?;
        Ok(())
    }
}
