//! Module continaing ERC20 token interaction implementations.

use {
    alloy::primitives::{Address, U256},
    contracts::alloy::ERC20,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::IntoLegacy,
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Debug)]
pub struct Erc20ApproveInteraction {
    pub token: ERC20::Instance,
    pub spender: Address,
    pub amount: U256,
}

impl Erc20ApproveInteraction {
    pub fn as_encoded(&self) -> EncodedInteraction {
        (
            self.token.address().into_legacy(),
            0.into(),
            Bytes(
                self.token
                    .approve(self.spender, self.amount)
                    .calldata()
                    .to_vec(),
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
            token: ERC20::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            spender: [0x02; 20].into(),
            amount: U256::from_be_bytes([0x03; 32]),
        };

        let (target, value, calldata) = approve.as_encoded();
        assert_eq!(target, approve.token.address().into_legacy());
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
