use {
    contracts::ethcontract::{H160, U256},
    model::TokenPair,
    shared::baseline_solver::BaselineSolvable,
};

#[derive(Debug)]
pub struct Pool {
    pub uni_v3_quoter_contract: contracts::UniswapV3QuoterV2,
    pub address: H160,
    pub tokens: TokenPair,
    pub fee: u32,
}

impl Pool {
    // Estimated with https://dune.com/queries/5431793
    const POOL_SWAP_GAS_COST: usize = 65_000;
    const QUOTER_V2_ADDRESS: H160 = shared::addr!("61fFE014bA17989E743c5F6cB21bF9697530B21e");
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
            .quote_exact_input_single((in_token, out_token, in_amount, self.fee, 0.into()))
            .call()
            .await
            .inspect_err(|err| {
                tracing::debug!(?err, "failed to get amount out from Uniswap V3 Quoter V2");
            })
            .map(
                |(amount_out, _sqrt_price_x96_after, _initialized_ticks_crossed, _gas_estimate)| {
                    amount_out
                },
            )
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
            .quote_exact_output_single((in_token, out_token, out_amount, self.fee, 0.into()))
            .call()
            .await
            .inspect_err(|err| {
                tracing::debug!(?err, "failed to get amount in from Uniswap V3 Quoter V2");
            })
            .map(
                |(amount_in, _sqrt_price_x96_after, _initialized_ticks_crossed, _gas_estimate)| {
                    amount_in
                },
            )
            .ok()
    }

    async fn gas_cost(&self) -> usize {
        Self::POOL_SWAP_GAS_COST
    }
}
