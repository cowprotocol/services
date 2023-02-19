//! Balancer boundary module. This contains encoding logic for the Balancer V2
//! vault contract. The reason this was moved to a boundary module is that it
//! currently requires `web3` as dependencies (not to mention needing to
//! instantiate a "dummy" web3 instance) for ABI encoding, which should **not**
//! be necessary at all. The swap parameter types are here instead of the domain
//! as they should be generated with the contract bindings (and are more like
//! DTOs than domain objects anyway).

pub use ethcontract::I256;
use {
    crate::domain::{dex, eth},
    contracts::BalancerV2Vault,
    ethcontract::Bytes,
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
