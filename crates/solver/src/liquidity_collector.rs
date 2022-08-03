use crate::{
    liquidity::Liquidity,
    liquidity::{
        balancer_v2::BalancerV2Liquidity, uniswap_v2::UniswapLikeLiquidity,
        uniswap_v3::UniswapV3Liquidity, zeroex::ZeroExLiquidity, LimitOrder,
    },
};
use anyhow::{Context, Result};
use shared::recent_block_cache::Block;

pub struct LiquidityCollector {
    pub uniswap_like_liquidity: Vec<UniswapLikeLiquidity>,
    pub balancer_v2_liquidity: Option<BalancerV2Liquidity>,
    pub zeroex_liquidity: Option<ZeroExLiquidity>,
    pub uniswap_v3_liquidity: Option<UniswapV3Liquidity>,
}

impl LiquidityCollector {
    pub async fn get_liquidity_for_orders(
        &self,
        limit_orders: &[LimitOrder],
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        let mut amms = vec![];
        let user_orders = limit_orders
            .iter()
            .filter(|order| !order.is_liquidity_order)
            .cloned()
            .collect::<Vec<_>>();
        for liquidity in &self.uniswap_like_liquidity {
            amms.extend(
                liquidity
                    .get_liquidity(&user_orders, at_block)
                    .await
                    .context("failed to get UniswapLike liquidity")?
                    .into_iter()
                    .map(Liquidity::ConstantProduct),
            );
        }
        if let Some(balancer_v2_liquidity) = self.balancer_v2_liquidity.as_ref() {
            let (stable_orders, weighted_orders) = balancer_v2_liquidity
                .get_liquidity(&user_orders, at_block)
                .await
                .context("failed to get Balancer liquidity")?;

            amms.extend(weighted_orders.into_iter().map(Liquidity::BalancerWeighted));
            amms.extend(stable_orders.into_iter().map(Liquidity::BalancerStable));
        }
        if let Some(zeroex_liquidity) = self.zeroex_liquidity.as_ref() {
            amms.append(&mut zeroex_liquidity.get_liquidity(limit_orders).await?)
        }
        if let Some(uniswap_v3_liquidity) = self.uniswap_v3_liquidity.as_ref() {
            amms.extend(
                uniswap_v3_liquidity
                    .get_liquidity(&user_orders)
                    .await?
                    .into_iter()
                    .map(Liquidity::Concentrated),
            )
        }
        tracing::debug!("got {} AMMs", amms.len());

        Ok(amms)
    }
}
