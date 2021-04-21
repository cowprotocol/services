use anyhow::{Context, Result};

use crate::{
    liquidity::uniswap::UniswapLikeLiquidity, liquidity::Liquidity, orderbook::OrderBookApi,
};

pub struct LiquidityCollector {
    pub uniswap_liquidity: UniswapLikeLiquidity,
    pub orderbook_api: OrderBookApi,
}

impl LiquidityCollector {
    pub async fn get_liquidity(&self) -> Result<Vec<Liquidity>> {
        let limit_orders = self
            .orderbook_api
            .get_liquidity()
            .await
            .context("failed to get orderbook")?;
        tracing::debug!("got {} orders", limit_orders.len());

        let amms = self
            .uniswap_liquidity
            .get_liquidity(limit_orders.iter())
            .await
            .context("failed to get uniswap pools")?;
        tracing::debug!("got {} AMMs", amms.len());

        Ok(limit_orders
            .into_iter()
            .map(Liquidity::Limit)
            .chain(amms.into_iter().map(Liquidity::Amm))
            .collect())
    }
}
