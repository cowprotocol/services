use {
    contracts::UniswapV3SwapRouter,
    ethcontract::Bytes,
    primitive_types::{H160, U256},
    shared::{
        http_solver::model::TokenAmount,
        interaction::{EncodedInteraction, Interaction},
    },
};

#[derive(Debug)]
pub struct UniswapV3Interaction {
    pub router: UniswapV3SwapRouter,
    pub params: ExactOutputSingleParams,
}

#[derive(Debug)]
pub struct ExactOutputSingleParams {
    pub token_amount_in_max: TokenAmount,
    pub token_amount_out: TokenAmount,
    pub fee: u32,
    pub recipient: H160,
    pub deadline: U256,
    pub sqrt_price_limit_x96: U256,
}
impl Interaction for UniswapV3Interaction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let method = self.router.exact_output_single((
            self.params.token_amount_in_max.token,
            self.params.token_amount_out.token,
            self.params.fee,
            self.params.recipient,
            self.params.deadline,
            self.params.token_amount_out.amount,
            self.params.token_amount_in_max.amount,
            self.params.sqrt_price_limit_x96,
        ));
        let calldata = method.tx.data.expect("no calldata").0;
        vec![(self.router.address(), 0.into(), Bytes(calldata))]
    }
}

#[cfg(test)]
mod tests {
    use {super::*, contracts::UniswapV3SwapRouter, hex_literal::hex, shared::dummy_contract};

    fn u8_as_32_bytes_be(u: u8) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[31] = u;
        result
    }

    #[test]
    fn encode_uniswap_call() {
        let amount_out = 5;
        let amount_in_max = 6;
        let token_in = 7;
        let token_out = 8;
        let fee = 10000;
        let payout_to = 9;
        let deadline = U256::MAX;
        let router = dummy_contract!(UniswapV3SwapRouter, H160::from_low_u64_be(4));
        let interaction = UniswapV3Interaction {
            router: router.clone(),
            params: ExactOutputSingleParams {
                token_amount_in_max: TokenAmount::new(
                    H160::from_low_u64_be(token_in.into()),
                    amount_in_max,
                ),
                token_amount_out: TokenAmount::new(
                    H160::from_low_u64_be(token_out.into()),
                    amount_out,
                ),
                fee,
                recipient: H160::from_low_u64_be(payout_to as u64),
                deadline,
                sqrt_price_limit_x96: U256::zero(),
            },
        };
        let interactions = interaction.encode();

        // Single interaction
        assert_eq!(interactions.len(), 1);

        // Verify Swap
        let swap_call = &interactions[0];
        assert_eq!(swap_call.0, router.address());
        let call = &swap_call.2 .0;
        let swap_signature = hex!("db3e2198");
        let deadline = [0xffu8; 32];
        assert_eq!(call[0..4], swap_signature);
        assert_eq!(call[4..36], u8_as_32_bytes_be(token_in));
        assert_eq!(call[36..68], u8_as_32_bytes_be(token_out));
        assert_eq!(call[96..100], fee.to_be_bytes());
        assert_eq!(call[100..132], u8_as_32_bytes_be(payout_to));
        assert_eq!(call[132..164], deadline);
        assert_eq!(call[164..196], u8_as_32_bytes_be(amount_out));
        assert_eq!(call[196..228], u8_as_32_bytes_be(amount_in_max));
        assert_eq!(call[228..260], u8_as_32_bytes_be(0));
    }
}
