use crate::encoding::EncodedInteraction;
use contracts::{BalancerV2Vault, GPv2Settlement};
use ethcontract::{Bytes, H160, H256};
use model::interaction::Interaction;
use primitive_types::U256;

#[derive(Clone, Debug)]
pub struct BalancerSwapGivenOutInteraction {
    pub settlement: GPv2Settlement,
    pub vault: BalancerV2Vault,
    pub pool_id: H256,
    pub asset_in: H160,
    pub asset_out: H160,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub user_data: Bytes<Vec<u8>>,
}

#[repr(u8)]
pub enum SwapKind {
    GivenIn = 0,
    GivenOut = 1,
}

lazy_static::lazy_static! {
    /// An impossibly distant future timestamp. Note that we use `0x80000...00`
    /// as the value so that it is mostly 0's to save small amounts of gas on
    /// calldata.
    pub static ref NEVER: U256 = U256::from(1) << 255;
}

impl Interaction for BalancerSwapGivenOutInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let method = self.vault.swap(
            (
                Bytes(self.pool_id.0),
                SwapKind::GivenOut as _,
                self.asset_in,
                self.asset_out,
                self.amount_out,
                self.user_data.clone(),
            ),
            (
                self.settlement.address(), // sender
                false,                     // fromInternalBalance
                self.settlement.address(), // recipient
                false,                     // toInternalBalance
            ),
            self.amount_in_max,
            *NEVER,
        );
        let calldata = method.tx.data.expect("no calldata").0;
        vec![(self.vault.address(), 0.into(), Bytes(calldata))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::dummy_contract;

    #[test]
    fn encode_unwrap_weth() {
        let vault = dummy_contract!(BalancerV2Vault, [0x01; 20]);
        let interaction = BalancerSwapGivenOutInteraction {
            settlement: dummy_contract!(GPv2Settlement, [0x02; 20]),
            vault: vault.clone(),
            pool_id: H256([0x03; 32]),
            asset_in: H160([0x04; 20]),
            asset_out: H160([0x05; 20]),
            amount_out: U256::from(42_000_000_000_000_000_000u128),
            amount_in_max: U256::from(1_337_000_000_000_000_000_000u128),
            user_data: Bytes::default(),
        };

        // Computed using Ethers.js:
        // ```js
        // vault.interface.encodeFunctionData("swap", [
        //   {
        //     poolId: "0x0303030303030303030303030303030303030303030303030303030303030303",
        //     kind: 1,
        //     assetIn: "0x0404040404040404040404040404040404040404",
        //     assetOut: "0x0505050505050505050505050505050505050505",
        //     amount: ethers.utils.parseEther("42.0"),
        //     userData: "0x",
        //   },
        //   {
        //     sender: "0x0202020202020202020202020202020202020202",
        //     fromInternalBalance: false,
        //     recipient: "0x0202020202020202020202020202020202020202",
        //     toInternalBalance: false,
        //   },
        //   ethers.utils.parseEther("1337.0"),
        //   "0x8000000000000000000000000000000000000000000000000000000000000000",
        // ])
        // ```
        assert_eq!(
            interaction.encode(),
            vec![(
                vault.address(),
                0.into(),
                Bytes(
                    hex::decode(
                        "52bbbe29\
                         00000000000000000000000000000000000000000000000000000000000000e0\
                         0000000000000000000000000202020202020202020202020202020202020202\
                         0000000000000000000000000000000000000000000000000000000000000000\
                         0000000000000000000000000202020202020202020202020202020202020202\
                         0000000000000000000000000000000000000000000000000000000000000000\
                         0000000000000000000000000000000000000000000000487a9a304539440000\
                         8000000000000000000000000000000000000000000000000000000000000000\
                         0303030303030303030303030303030303030303030303030303030303030303\
                         0000000000000000000000000000000000000000000000000000000000000001\
                         0000000000000000000000000404040404040404040404040404040404040404\
                         0000000000000000000000000505050505050505050505050505050505050505\
                         00000000000000000000000000000000000000000000000246ddf97976680000\
                         00000000000000000000000000000000000000000000000000000000000000c0\
                         0000000000000000000000000000000000000000000000000000000000000000"
                    )
                    .unwrap()
                ),
            )]
        );
    }
}
