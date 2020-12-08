use crate::{ethereum::SettlementContract, naive_solver, orderbook::OrderBookApi};
use anyhow::{Context, Result};
use std::{sync::Arc, time::Duration};

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    contract: Arc<dyn SettlementContract>,
    orderbook: OrderBookApi,
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

    async fn single_run(&mut self) -> Result<()> {
        let orders = self.orderbook.get_orders().await?;
        // TODO: order validity checks
        // Decide what is handled by orderbook service and what by us.
        // We likely want to at least mark orders we know we have settled so that we don't
        // attempt to settle them again when they are still in the orderbook.
        let settlement =
            match naive_solver::settle(orders.into_iter().map(|order| order.order_creation)) {
                None => return Ok(()),
                Some(settlement) => settlement,
            };
        // TODO: encode settlement, check if we need to approve spending to uniswap
        // TODO: use retry transaction sending crate for updating gas prices
        self.contract
            .settle_call(&settlement)
            .await
            .context("settle call failed")?;
        self.contract
            .settle_send(&settlement)
            .await
            .context("settle send failed")?;
        Ok(())
    }
}
