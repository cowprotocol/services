//! Module continaing ERC20 token interaction implementations.

use {
    alloy::{
        primitives::{Address, U256},
        sol_types::SolCall,
    },
    contracts::ERC20,
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Debug)]
pub struct Erc20ApproveInteraction {
    pub token: Address,
    pub spender: Address,
    pub amount: U256,
}

impl Erc20ApproveInteraction {
    pub fn as_encoded(&self) -> EncodedInteraction {
        (
            self.token,
            U256::ZERO,
            ERC20::ERC20::approveCall {
                spender: self.spender,
                amount: self.amount,
            }
            .abi_encode()
            .into(),
        )
    }
}

impl Interaction for Erc20ApproveInteraction {
    fn encode(&self) -> EncodedInteraction {
        self.as_encoded()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn encode_erc20_approve() {
        let approve = Erc20ApproveInteraction {
            token: [0x01; 20].into(),
            spender: [0x02; 20].into(),
            amount: U256::from_be_bytes([0x03; 32]),
        };

        let (target, value, calldata) = approve.as_encoded();
        assert_eq!(target, approve.token);
        assert!(value.is_zero());
        assert_eq!(
            calldata.0,
            hex!(
                "095ea7b3
                 0000000000000000000000000202020202020202020202020202020202020202
                 0303030303030303030303030303030303030303030303030303030303030303"
            )
            .to_vec()
        );
    }
}
