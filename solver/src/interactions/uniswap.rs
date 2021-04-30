use super::Erc20ApproveInteraction;
use crate::{encoding::EncodedInteraction, settlement::Interaction};
use contracts::{GPv2Settlement, IUniswapLikeRouter, ERC20};
use primitive_types::{H160, U256};

#[derive(Debug)]
pub struct UniswapInteraction {
    pub router: IUniswapLikeRouter,
    pub settlement: GPv2Settlement,
    pub set_allowance: bool,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub token_in: H160,
    pub token_out: H160,
}

impl Interaction for UniswapInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let mut result = Vec::new();
        if self.set_allowance {
            result.push(self.encode_approve());
        }
        result.push(self.encode_swap());
        result
    }
}

impl UniswapInteraction {
    fn encode_approve(&self) -> EncodedInteraction {
        let token = ERC20::at(&self.web3(), self.token_in);
        Erc20ApproveInteraction {
            token,
            spender: self.router.address(),
            amount: U256::MAX,
        }
        .as_encoded()
    }

    fn encode_swap(&self) -> EncodedInteraction {
        let method = self.router.swap_tokens_for_exact_tokens(
            self.amount_out,
            self.amount_in_max,
            vec![self.token_in, self.token_out],
            self.settlement.address(),
            U256::MAX,
        );
        let calldata = method.tx.data.expect("no calldata").0;
        (self.router.address(), 0.into(), calldata)
    }

    fn web3(&self) -> web3::Web3<ethcontract::transport::DynTransport> {
        self.settlement.raw_instance().web3()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::dummy_web3;
    use contracts::IUniswapLikeRouter;
    use hex_literal::hex;

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
        let router = IUniswapLikeRouter::at(&dummy_web3::dummy_web3(), H160::from_low_u64_be(4));
        let settlement = GPv2Settlement::at(
            &dummy_web3::dummy_web3(),
            H160::from_low_u64_be(payout_to as u64),
        );
        let interaction = UniswapInteraction {
            router: router.clone(),
            settlement,
            set_allowance: true,
            amount_out: amount_out.into(),
            amount_in_max: amount_in_max.into(),
            token_in,
            token_out: H160::from_low_u64_be(token_out as u64),
        };
        let interactions = interaction.encode();

        // Verify Approve
        let approve_call = &interactions[0];
        assert_eq!(approve_call.0, token_in);
        let call = &approve_call.2;
        let approve_signature = hex!("095ea7b3");
        assert_eq!(call[0..4], approve_signature);
        assert_eq!(&call[16..36], router.address().as_fixed_bytes()); //spender
        assert_eq!(call[36..68], [0xffu8; 32]); // amount

        // Verify Swap
        let swap_call = &interactions[1];
        assert_eq!(swap_call.0, router.address());
        let call = &swap_call.2;
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
