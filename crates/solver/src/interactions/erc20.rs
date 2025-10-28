//! Module continaing ERC20 token interaction implementations.

use {
    alloy::{
        primitives::{Address, U256},
        sol_types::SolCall,
    },
    contracts::alloy::ERC20,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::IntoLegacy,
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
            self.token.into_legacy(),
            0.into(),
            Bytes(
                ERC20::ERC20::approveCall {
                    spender: self.spender,
                    amount: self.amount,
                }
                .abi_encode(),
            ),
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
    use {super::*, ethrpc::alloy::conversions::IntoLegacy, hex_literal::hex};

    #[test]
    fn encode_erc20_approve() {
        let approve = Erc20ApproveInteraction {
            token: [0x01; 20].into(),
            spender: [0x02; 20].into(),
            amount: U256::from_be_bytes([0x03; 32]),
        };

        let (target, value, calldata) = approve.as_encoded();
        assert_eq!(target, approve.token.into_legacy());
        assert_eq!(value, 0.into());
        assert_eq!(
            calldata.0,
            hex!(
                "095ea7b3
                 0000000000000000000000000202020202020202020202020202020202020202
                 0303030303030303030303030303030303030303030303030303030303030303"
            )
        );
    }
}
