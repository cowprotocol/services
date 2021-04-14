use crate::{encoding::EncodedInteraction, settlement::Interaction};
use contracts::WETH9;
use primitive_types::U256;

#[derive(Debug)]
pub struct UnwrapWethInteraction {
    pub weth: WETH9,
    pub amount: U256,
}

impl Interaction for UnwrapWethInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let method = self.weth.withdraw(self.amount);
        let calldata = method.tx.data.expect("no calldata").0;
        vec![(self.weth.address(), 0.into(), calldata)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::dummy_web3;
    use ethcontract::H160;
    use hex_literal::hex;

    #[test]
    fn encode_unwrap_weth() {
        let weth = WETH9::at(&dummy_web3::dummy_web3(), H160([0x42; 20]));
        let amount = U256::from(13_370_000_000_000_000_000u128);
        let interaction = UnwrapWethInteraction {
            weth: weth.clone(),
            amount,
        };
        let encoded_interactions = interaction.encode();

        let withdraw_call = &encoded_interactions[0];
        assert_eq!(withdraw_call.0, weth.address());
        assert_eq!(withdraw_call.1, U256::from(0));
        let call = &withdraw_call.2;
        assert_eq!(call.len(), 36);
        let withdraw_signature = hex!("2e1a7d4d");
        assert_eq!(call[0..4], withdraw_signature);
        assert_eq!(U256::from_big_endian(&call[4..36]), amount);
    }
}
