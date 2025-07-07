use {
    contracts::{
        ethcontract::{H160, I256, U256},
        uniswap_v3_quoter_v2,
    },
    ethrpc::Web3,
    model::TokenPair,
    shared::baseline_solver::BaselineSolvable,
};

#[derive(Debug)]
pub struct Pool {
    pub web3: Web3,
    pub address: H160,
    pub tokens: TokenPair,
    pub fee: u32,
}

impl Pool {
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
        // let in_amount = I256::from_raw(in_amount);
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // tracing::error!(neg = in_amount.is_negative(), "abort");
            // pool has wrong tokens or input amount would overflow
            return None;
        }

        let contract = uniswap_v3_quoter_v2::Contract::at(
            &self.web3,
            Self::QUOTER_V2_ADDRESS,
        );
        contract
            .quote_exact_input_single((in_token, out_token, in_amount, self.fee, 0.into()))
            .call()
            .await
            // todo: inspect error
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
        // let out_amount = I256::from_raw(out_amount);
        if TokenPair::new(out_token, in_token) != Some(self.tokens) {
            // tracing::error!(neg = out_amount.is_negative(), "abort");
            // pool has wrong tokens or out amount would overflow
            return None;
        }

        let contract = uniswap_v3_quoter_v2::Contract::at(
            &self.web3,
            Self::QUOTER_V2_ADDRESS,
        );
        contract
            .quote_exact_output_single((in_token, out_token, out_amount, self.fee, 0.into()))
            .call()
            .await
            // todo: inspect error
            .map(
                |(amount_in, _sqrt_price_x96_after, _initialized_ticks_crossed, _gas_estimate)| {
                    amount_in
                },
            )
            .ok()
    }

    async fn gas_cost(&self) -> usize {
        // TODO: research a reasonable approximation
        100_000
    }
}

fn abs(val: &I256) -> U256 {
    let mut bytes = [0_u8; 32];
    val.abs().to_big_endian(&mut bytes);
    U256::from_big_endian(&bytes)
}

fn price_limit(zero_for_one: bool) -> U256 {
    match zero_for_one {
        true => 4295128740u128.into(),
        false => U256::from_dec_str("1461446703485210103287273052203988822378723970341").unwrap(),
    }
}
