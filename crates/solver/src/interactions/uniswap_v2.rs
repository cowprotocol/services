use {
    alloy::{primitives::Address, sol_types::SolCall},
    contracts::alloy::IUniswapLikeRouter,
    ethrpc::alloy::conversions::IntoAlloy,
    primitive_types::{H160, U256},
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Debug)]
pub struct UniswapInteraction {
    pub router: Address,
    pub settlement: Address,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub token_in: H160,
    pub token_out: H160,
}

impl Interaction for UniswapInteraction {
    fn encode(&self) -> EncodedInteraction {
        self.encode_swap()
    }
}

impl UniswapInteraction {
    pub fn encode_swap(&self) -> EncodedInteraction {
        let calldata = IUniswapLikeRouter::IUniswapLikeRouter::swapTokensForExactTokensCall {
            amountOut: self.amount_out.into_alloy(),
            amountInMax: self.amount_in_max.into_alloy(),
            path: vec![self.token_in.into_alloy(), self.token_out.into_alloy()],
            to: self.settlement,
            deadline: ::alloy::primitives::U256::MAX,
        }
        .abi_encode();
        (self.router, alloy::primitives::U256::ZERO, calldata.into())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::Address, hex_literal::hex};

    fn u8_as_32_bytes_be(u: u8) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[31] = u;
        result
    }

    #[test]
    fn encode_uniswap_call() {
        let amount_out = 5;
        let amount_in_max = 6;
        let token_in = H160::from_low_u64_be(7);
        let token_out = 8;
        let payout_to = 9u8;

        let router_address = Address::from(&[1u8; 20]);
        let settlement = H160::from_low_u64_be(payout_to as u64).into_alloy();
        let interaction = UniswapInteraction {
            router: router_address,
            settlement,
            amount_out: amount_out.into(),
            amount_in_max: amount_in_max.into(),
            token_in,
            token_out: H160::from_low_u64_be(token_out as u64),
        };
        let swap_call = interaction.encode();

        // Verify Swap
        assert_eq!(swap_call.0, router_address);
        let call = &swap_call.2.0;
        let swap_signature = hex!("8803dbee");
        let path_offset = 160;
        let path_size = 2;
        let deadline = [0xffu8; 32];
        assert_eq!(call[0..4], swap_signature);
        assert_eq!(call[4..36], u8_as_32_bytes_be(amount_out));
        assert_eq!(call[36..68], u8_as_32_bytes_be(amount_in_max));
        assert_eq!(call[68..100], u8_as_32_bytes_be(path_offset));
        assert_eq!(call[100..132], u8_as_32_bytes_be(payout_to));
        assert_eq!(call[132..164], deadline);
        assert_eq!(call[164..196], u8_as_32_bytes_be(path_size));
        assert_eq!(&call[208..228], token_in.as_fixed_bytes());
        assert_eq!(call[228..260], u8_as_32_bytes_be(token_out));
    }
}
