use crate::{naive_solver, orderbook::OrderBookApi};
use anyhow::{anyhow, Context, Result};
use contracts::{GPv2Settlement, UniswapV2Router02};
use std::time::Duration;

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    pub settlement_contract: GPv2Settlement,
    pub uniswap_contract: UniswapV2Router02,
    pub orderbook: OrderBookApi,
}

impl Driver {
    async fn run_forever(&mut self) {
        loop {
            println!("starting settle attempt");
            match self.single_run().await {
                Ok(()) => println!("ok"),
                Err(err) => println!("error: {:?}", err),
            }
            tokio::time::delay_for(SETTLE_INTERVAL).await;
        }
    }

    pub async fn single_run(&mut self) -> Result<()> {
        let orders = self.orderbook.get_orders().await?;
        // TODO: order validity checks
        // Decide what is handled by orderbook service and what by us.
        // We likely want to at least mark orders we know we have settled so that we don't
        // attempt to settle them again when they are still in the orderbook.
        let settlement = match naive_solver::settle(
            orders.into_iter().map(|order| order.order_creation),
            &self.uniswap_contract,
            &self.settlement_contract.address(),
        ) {
            None => return Ok(()),
            Some(settlement) => settlement,
        };
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
        settle().call().await.context("settle simulation failed")?;
        settle().send().await.context("settle execution failed")?;
        Ok(())
    }
}
