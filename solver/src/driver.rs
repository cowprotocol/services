use crate::{naive_solver, orderbook::OrderBookApi};
use anyhow::{anyhow, Context, Result};
use contracts::{GPv2Settlement, UniswapV2Router02};
use std::time::Duration;
use tracing::info;

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    pub settlement_contract: GPv2Settlement,
    pub uniswap_contract: UniswapV2Router02,
    pub orderbook: OrderBookApi,
}

impl Driver {
    pub fn new(
        settlement_contract: GPv2Settlement,
        uniswap_contract: UniswapV2Router02,
        orderbook: OrderBookApi,
    ) -> Self {
        Self {
            settlement_contract,
            uniswap_contract,
            orderbook,
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
        let orders = self
            .orderbook
            .get_orders()
            .await
            .context("failed to get orderbook")?;
        tracing::debug!("got {} orders", orders.len());
        // TODO: order validity checks
        // Decide what is handled by orderbook service and what by us.
        // We likely want to at least mark orders we know we have settled so that we don't
        // attempt to settle them again when they are still in the orderbook.
        let settlement = match naive_solver::settle(
            orders.into_iter().map(|order| order.order_creation),
            &self.uniswap_contract,
            &self.settlement_contract,
        ) {
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
