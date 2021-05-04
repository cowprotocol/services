//! Module continaing ERC20 token interaction implementations.

use crate::{encoding::EncodedInteraction, settlement::Interaction};
use contracts::ERC20;
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
        (self.token.address(), 0.into(), calldata)
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
    use crate::testutil;
    use hex_literal::hex;

    #[test]
    fn encode_erc20_approve() {
        let approve = Erc20ApproveInteraction {
            token: ERC20::at(&testutil::dummy_web3(), H160([0x01; 20])),
            spender: H160([0x02; 20]),
            amount: U256::from_big_endian(&[0x03; 32]),
        };

        let (target, value, calldata) = approve.as_encoded();
        assert_eq!(target, approve.token.address());
        assert_eq!(value, 0.into());
        assert_eq!(
            calldata,
            hex!(
                "095ea7b3
                 0000000000000000000000000202020202020202020202020202020202020202
                 0303030303030303030303030303030303030303030303030303030303030303"
            )
        );
    }
}
