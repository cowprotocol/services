use crate::liquidity::{uniswap::UniswapLiquidity, Liquidity};
use crate::{orderbook::OrderBookApi, solver::Solver};
use anyhow::{anyhow, Context, Result};
use contracts::GPv2Settlement;
use std::time::Duration;
use tracing::info;

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    settlement_contract: GPv2Settlement,
    orderbook: OrderBookApi,
    uniswap_liquidity: UniswapLiquidity,
    solver: Box<dyn Solver>,
}

impl Driver {
    pub fn new(
        settlement_contract: GPv2Settlement,
        uniswap_liquidity: UniswapLiquidity,
        orderbook: OrderBookApi,
        solver: Box<dyn Solver>,
    ) -> Self {
        Self {
            settlement_contract,
            solver,
            orderbook,
            uniswap_liquidity,
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
        // TODO: check if we need to approve spending to uniswap
        // TODO: use retry transaction sending crate for updating gas prices
        let encoded_interactions = settlement
            .encode_interactions()
            .context("interaction encoding failed")?;
        let encoded_trades = settlement
            .encode_trades()
            .ok_or_else(|| anyhow!("trade encoding failed"))?;
        let settle = || {
            self.settlement_contract
                .settle(
                    settlement.tokens(),
                    settlement.clearing_prices(),
                    encoded_trades.clone(),
                    encoded_interactions.clone(),
                    Vec::new(),
                )
                .gas(8_000_000u32.into())
        };
        tracing::info!(
            "Settlement call: {}",
            hex::encode(settle().tx.data.expect("data").0),
        );
        settle().call().await.context("settle simulation failed")?;
        settle().send().await.context("settle execution failed")?;
        Ok(())
    }
}
