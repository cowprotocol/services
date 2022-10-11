//! Module continaing ERC20 token interaction implementations.

use crate::encoding::EncodedInteraction;
use contracts::ERC20;
use ethcontract::Bytes;
use model::interaction::Interaction;
use primitive_types::{H160, U256};

#[derive(Debug)]
pub struct Erc20ApproveInteraction {
    pub token: ERC20,
    pub spender: H160,
    pub amount: U256,
}

impl Erc20ApproveInteraction {
    pub fn as_encoded(&self) -> EncodedInteraction {
        let method = self.token.approve(self.spender, self.amount);
        let calldata = method.tx.data.expect("no calldata").0;
        (self.token.address(), 0.into(), Bytes(calldata))
    }
}

impl Interaction for Erc20ApproveInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![self.as_encoded()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use shared::dummy_contract;

    #[test]
    fn encode_erc20_approve() {
        let approve = Erc20ApproveInteraction {
            token: dummy_contract!(ERC20, [0x01; 20]),
            spender: H160([0x02; 20]),
            amount: U256::from_big_endian(&[0x03; 32]),
        };

        let (target, value, calldata) = approve.as_encoded();
        assert_eq!(target, approve.token.address());
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
