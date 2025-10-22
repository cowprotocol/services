use {
    alloy::primitives::aliases::U24,
    contracts::{
        alloy::UniswapV3QuoterV2::IQuoterV2::QuoteExactInputSingleParams,
        ethcontract::{H160, U256},
    },
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    model::TokenPair,
    shared::baseline_solver::BaselineSolvable,
    std::sync::Arc,
};

#[derive(Debug)]
pub struct Pool {
    pub uni_v3_quoter_contract: Arc<contracts::alloy::UniswapV3QuoterV2::Instance>,
    pub address: H160,
    pub tokens: TokenPair,
    pub fee: U24,
}

impl Pool {
    // Estimated with https://dune.com/queries/5431793
    const POOL_SWAP_GAS_COST: usize = 106_000;
}

/// Computes input or output amounts via eth_calls. The implementation was based
/// on these [docs](https://docs.uniswap.org/contracts/v3/reference/core/UniswapV3Pool#swap).
impl BaselineSolvable for Pool {
    async fn get_amount_out(
        &self,
        out_token: H160,
        (in_amount, in_token): (U256, H160),
    ) -> Option<U256> {
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // The pool has wrong tokens or input amount would overflow
            return None;
        }

        self.uni_v3_quoter_contract
            .quoteExactInputSingle(QuoteExactInputSingleParams {
                tokenIn: in_token.into_alloy(),
                tokenOut: out_token.into_alloy(),
                amountIn: in_amount.into_alloy(),
                fee: self.fee,
                sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
            })
            .call()
            .await
            .map(|result| result.amountOut.into_legacy())
            .ok()
    }

    async fn get_amount_in(
        &self,
        in_token: H160,
        (out_amount, out_token): (U256, H160),
    ) -> Option<U256> {
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // The pool has wrong tokens or out amount would overflow
            return None;
        }

        self.uni_v3_quoter_contract
            .quoteExactInputSingle(QuoteExactInputSingleParams {
                tokenIn: in_token.into_alloy(),
                tokenOut: out_token.into_alloy(),
                amountIn: out_amount.into_alloy(),
                fee: self.fee,
                sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
            })
            .call()
            .await
            .map(|result| result.amountOut.into_legacy())
            .ok()
    }

    async fn gas_cost(&self) -> usize {
        Self::POOL_SWAP_GAS_COST
    }
}
