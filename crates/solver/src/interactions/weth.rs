use {
    alloy::primitives::U256,
    anyhow::{Result, ensure},
    contracts::alloy::WETH9,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::IntoLegacy,
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Clone, Debug)]
pub struct UnwrapWethInteraction {
    pub weth: WETH9::Instance,
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
    fn encode(&self) -> EncodedInteraction {
        (
            self.weth.address().into_legacy(),
            0.into(),
            Bytes(self.weth.withdraw(self.amount).calldata().to_vec()),
        )
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn encode_unwrap_weth() {
        let weth = WETH9::Instance::new([0x42; 20].into(), ethrpc::mock::web3().alloy);
        let amount = U256::from(13_370_000_000_000_000_000u128);
        let interaction = UnwrapWethInteraction {
            weth: weth.clone(),
            amount,
        };
        let withdraw_call = interaction.encode();

        assert_eq!(withdraw_call.0, weth.address().into_legacy());
        assert_eq!(withdraw_call.1, U256::ZERO.into_legacy());
        let call = &withdraw_call.2.0;
        assert_eq!(call.len(), 36);
        let withdraw_signature = hex!("2e1a7d4d");
        assert_eq!(call[0..4], withdraw_signature);
        assert_eq!(U256::from_be_slice(&call[4..36]), amount);
    }

    #[test]
    fn merge_same_native_token() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::ONE,
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::from(2),
        };

        assert!(unwrap0.merge(&unwrap1).is_ok());
        assert_eq!(unwrap0.amount, U256::from(3));
    }

    #[test]
    fn merge_different_native_token() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::ONE,
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x02; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::from(2),
        };

        assert!(unwrap0.merge(&unwrap1).is_err());
        assert_eq!(unwrap0.amount, U256::ONE);
    }

    #[test]
    #[should_panic]
    fn merge_u256_overflow() {
        let mut unwrap0 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::ONE,
        };
        let unwrap1 = UnwrapWethInteraction {
            weth: WETH9::Instance::new([0x01; 20].into(), ethrpc::mock::web3().alloy),
            amount: U256::MAX,
        };

        let _ = unwrap0.merge(&unwrap1);
    }
}
