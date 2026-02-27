use {
    alloy::primitives::{Address, U256, aliases::U24},
    contracts::UniswapV3QuoterV2::IQuoterV2::{
        QuoteExactInputSingleParams, QuoteExactOutputSingleParams,
    },
    model::TokenPair,
    shared::baseline_solver::BaselineSolvable,
    std::sync::Arc,
};

#[derive(Debug)]
pub struct Pool {
    pub uni_v3_quoter_contract: Arc<contracts::UniswapV3QuoterV2::Instance>,
    pub address: Address,
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
        out_token: Address,
        (in_amount, in_token): (U256, Address),
    ) -> Option<U256> {
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // The pool has wrong tokens or input amount would overflow
            return None;
        }

        self.uni_v3_quoter_contract
            .quoteExactInputSingle(QuoteExactInputSingleParams {
                tokenIn: in_token,
                tokenOut: out_token,
                amountIn: in_amount,
                fee: self.fee,
                sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
            })
            .call()
            .await
            .map(|result| result.amountOut)
            .ok()
    }

    async fn get_amount_in(
        &self,
        in_token: Address,
        (out_amount, out_token): (U256, Address),
    ) -> Option<U256> {
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // The pool has wrong tokens or out amount would overflow
            return None;
        }

        self.uni_v3_quoter_contract
            .quoteExactOutputSingle(QuoteExactOutputSingleParams {
                tokenIn: in_token,
                tokenOut: out_token,
                amount: out_amount,
                fee: self.fee,
                sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
            })
            .call()
            .await
            .map(|result| result.amountIn)
            .ok()
    }

    async fn gas_cost(&self) -> usize {
        Self::POOL_SWAP_GAS_COST
    }
}
