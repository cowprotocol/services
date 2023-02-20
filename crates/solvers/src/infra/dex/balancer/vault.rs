//! This module contains logic for encoding swaps with the Balanver V2 Smart
//! Contract. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::{dex, eth},
    contracts::{
        ethcontract::{Bytes, I256},
        BalancerV2Vault,
    },
    ethereum_types::{H160, H256, U256},
};

pub struct Vault(BalancerV2Vault);

#[repr(u8)]
pub enum SwapKind {
    GivenIn = 0,
    GivenOut = 1,
}

pub struct Swap {
    pub pool_id: H256,
    pub asset_in_index: U256,
    pub asset_out_index: U256,
    pub amount: U256,
    pub user_data: Vec<u8>,
}

pub struct Funds {
    pub sender: H160,
    pub from_internal_balance: bool,
    pub recipient: H160,
    pub to_internal_balance: bool,
}

impl Vault {
    pub fn new(address: eth::ContractAddress) -> Self {
        Self(shared::dummy_contract!(BalancerV2Vault, address.0))
    }

    pub fn address(&self) -> eth::ContractAddress {
        eth::ContractAddress(self.0.address())
    }

    pub fn batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
        limits: Vec<I256>,
        deadline: U256,
    ) -> dex::Call {
        dex::Call {
            to: self.address(),
            calldata: self
                .0
                .methods()
                .batch_swap(
                    kind as _,
                    swaps
                        .into_iter()
                        .map(|swap| {
                            (
                                Bytes(swap.pool_id.0),
                                swap.asset_in_index,
                                swap.asset_out_index,
                                swap.amount,
                                Bytes(swap.user_data),
                            )
                        })
                        .collect(),
                    assets,
                    (
                        funds.sender,
                        funds.from_internal_balance,
                        funds.recipient,
                        funds.to_internal_balance,
                    ),
                    limits,
                    deadline,
                )
                .tx
                .data
                .expect("calldata")
                .0,
        }
    }
}
