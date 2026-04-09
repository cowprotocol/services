use {
    alloy::{
        primitives::{Address, B256, Bytes, U256},
        sol_types::SolCall,
    },
    contracts::BalancerV2Vault::{BalancerV2Vault::swapCall, IVault},
    shared::{
        http_solver::model::TokenAmount,
        interaction::{EncodedInteraction, Interaction},
    },
    std::sync::LazyLock,
};

#[derive(Clone, Debug)]
pub struct BalancerSwapGivenOutInteraction {
    pub settlement: Address,
    pub vault: Address,
    pub pool_id: B256,
    pub asset_in_max: TokenAmount,
    pub asset_out: TokenAmount,
    pub user_data: Bytes,
}

/// An impossibly distant future timestamp. Note that we use `0x80000...00`
/// as the value so that it is mostly 0's to save small amounts of gas on
/// calldata.
pub static NEVER: LazyLock<U256> = LazyLock::new(|| U256::from(1) << 255);

impl BalancerSwapGivenOutInteraction {
    pub fn encode_swap(&self) -> EncodedInteraction {
        let single_swap = IVault::SingleSwap {
            poolId: self.pool_id,
            kind: 1, // GivenOut
            assetIn: self.asset_in_max.token,
            assetOut: self.asset_out.token,
            amount: self.asset_out.amount,
            userData: self.user_data.clone(),
        };
        let funds = IVault::FundManagement {
            sender: self.settlement,
            fromInternalBalance: false,
            recipient: self.settlement,
            toInternalBalance: false,
        };

        let method = swapCall {
            singleSwap: single_swap,
            funds,
            limit: self.asset_in_max.amount,
            deadline: *NEVER,
        }
        .abi_encode();

        (self.vault, U256::ZERO, method.into())
    }
}

impl Interaction for BalancerSwapGivenOutInteraction {
    fn encode(&self) -> EncodedInteraction {
        self.encode_swap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_unwrap_weth() {
        let vault_address = [0x01; 20].into();
        let interaction = BalancerSwapGivenOutInteraction {
            settlement: Address::from_slice(&[0x02; 20]),
            vault: vault_address,
            pool_id: B256::repeat_byte(0x03),
            asset_in_max: TokenAmount::new(
                Address::repeat_byte(0x04),
                alloy::primitives::U256::from(1_337_000_000_000_000_000_000u128),
            ),
            asset_out: TokenAmount::new(
                Address::repeat_byte(0x05),
                alloy::primitives::U256::from(42_000_000_000_000_000_000u128),
            ),
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
            (
                vault_address,
                U256::ZERO,
                const_hex::decode(
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
                .into()
            )
        );
    }
}
