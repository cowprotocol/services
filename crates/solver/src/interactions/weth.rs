use crate::encoding::EncodedInteraction;
use anyhow::{ensure, Result};
use contracts::WETH9;
use ethcontract::Bytes;
use model::interaction::Interaction;
use primitive_types::U256;

#[derive(Clone, Debug)]
pub struct UnwrapWethInteraction {
    pub weth: WETH9,
    pub amount: U256,
}

impl UnwrapWethInteraction {
    /// Tries to merge the specified unwrap with the current one, returning
    /// `true` if the merge was successful, and `false` otherwise.
    ///
    /// Returns an error on arithmetic overflow.
    pub fn merge(&mut self, other: &Self) -> Result<()> {
        ensure!(
            self.weth.address() == other.weth.address(),
            "cannot merge unwrap for different token addresses",
        );

        self.amount = self
            .amount
            .checked_add(other.amount)
            .expect("no one is that rich");
        Ok(())
    }
}

impl Interaction for UnwrapWethInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let method = self.weth.withdraw(self.amount);
        let calldata = method.tx.data.expect("no calldata").0;
        vec![(self.weth.address(), 0.into(), Bytes(calldata))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use shared::dummy_contract;

    #[test]
    fn encode_unwrap_weth() {
        let weth = dummy_contract!(WETH9, [0x42; 20]);
        let amount = U256::from(13_370_000_000_000_000_000u128);
        let interaction = UnwrapWethInteraction {
            weth: weth.clone(),
            amount,
        };
        let encoded_interactions = interaction.encode();

        let withdraw_call = &encoded_interactions[0];
        assert_eq!(withdraw_call.0, weth.address());
        assert_eq!(withdraw_call.1, U256::from(0));
        let call = &withdraw_call.2 .0;
        assert_eq!(call.len(), 36);
        let withdraw_signature = hex!("2e1a7d4d");
        assert_eq!(call[0..4], withdraw_signature);
        assert_eq!(U256::from_big_endian(&call[4..36]), amount);
    }

    #[test]
    fn merge_same_native_token() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: 1.into(),
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: 2.into(),
        };

        assert!(unwrap0.merge(&unwrap1).is_ok());
        assert_eq!(unwrap0.amount, 3.into());
    }

    #[test]
    fn merge_different_native_token() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: 1.into(),
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x02; 20]),
            amount: 2.into(),
        };

        assert!(unwrap0.merge(&unwrap1).is_err());
        assert_eq!(unwrap0.amount, 1.into());
    }

    #[test]
    #[should_panic]
    fn merge_u256_overflow() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: 1.into(),
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: U256::max_value(),
        };

        let _ = unwrap0.merge(&unwrap1);
    }
}
