use crate::{naive_solver, orderbook::OrderBookApi};
use anyhow::{Context, Result};
use contracts::{GPv2Settlement, UniswapV2Router02};
use std::time::Duration;

const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Driver {
    settlement_contract: GPv2Settlement,
    uniswap_router: UniswapV2Router02,
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
        // TODO: check if we need to approve spending to uniswap
        // TODO: use retry transaction sending crate for updating gas prices
        let settle = || {
            self.settlement_contract.settle(
                settlement.tokens(),
                settlement.clearing_prices(),
                settlement.fee_factor,
                settlement
                    .encode_trades()
                    .expect("naive solver created invalid settlement"),
                settlement.encode_interactions(
                    &self.uniswap_router.address(),
                    &self.settlement_contract.address(),
                ),
                Vec::new(),
            )
        };
        settle().call().await.context("settle simulation failed")?;
        settle().send().await.context("settle execution failed")?;
        Ok(())
    }
}
