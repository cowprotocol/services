use {
    crate::{
        domain::{dex, eth},
        infra::blockchain,
    },
    alloy::{
        primitives::{Address, U256},
        providers::DynProvider,
        rpc::types::state::{AccountOverride, StateOverridesBuilder},
    },
    contracts::support::{
        AnyoneAuthenticator,
        Swapper::{
            self,
            Swapper::{Allowance, Asset, Interaction},
        },
    },
};

/// A DEX swap simulator.
#[derive(Debug, Clone)]
pub struct Simulator {
    web3: DynProvider,
    settlement: Address,
    authenticator: Address,
}

impl Simulator {
    /// Create a new simulator for computing DEX swap gas usage.
    pub fn new(url: &reqwest::Url, settlement: Address, authenticator: Address) -> Self {
        Self {
            web3: blockchain::rpc(url).provider,
            settlement,
            authenticator,
        }
    }

    /// Simulate the gas needed by a single order DEX swap.
    ///
    /// This will return a `None` if the gas simulation is unavailable.
    pub async fn gas(&self, owner: Address, swap: &dex::Swap) -> Result<eth::Gas, Error> {
        if owner == self.settlement {
            // we can't have both the settlement and swapper contracts at the
            // same address
            return Err(Error::SettlementContractIsOwner);
        }

        let swapper = Swapper::Instance::new(owner, self.web3.clone());
        let overrides = StateOverridesBuilder::with_capacity(2)
            // Setup up our trader code that actually executes the settlement
            .append(
                *swapper.address(),
                AccountOverride {
                    code: Some(Swapper::Swapper::DEPLOYED_BYTECODE.clone()),
                    ..Default::default()
                },
            )
            // Override the CoW protocol solver authenticator with one that
            // allows any address to solve
            .append(
                self.authenticator,
                AccountOverride {
                    code: Some(
                        AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE.clone(),
                    ),
                    ..Default::default()
                },
            );

        let swapper_calls_arg = swap
            .calls
            .iter()
            .map(|call| Interaction {
                target: call.to,
                value: U256::ZERO,
                callData: alloy::primitives::Bytes::copy_from_slice(&call.calldata),
            })
            .collect();
        let sell = Asset {
            token: swap.input.token.0,
            amount: swap.input.amount,
        };
        let buy = Asset {
            token: swap.output.token.0,
            amount: swap.output.amount,
        };
        let allowance = Allowance {
            spender: swap.allowance.spender,
            amount: swap.allowance.amount.get(),
        };
        let gas = swapper
            .swap(self.settlement, sell, buy, allowance, swapper_calls_arg)
            .call()
            .overrides(overrides)
            .await?;

        // `gas == 0` means that the simulation is not possible. See
        // `Swapper.sol` contract for more details. In this case, use the
        // heuristic gas amount from the swap.
        Ok(if gas.is_zero() {
            tracing::info!(
                gas = ?swap.gas,
                "could not simulate dex swap to get gas used; fall back to gas estimate provided \
                 by dex API"
            );
            swap.gas
        } else {
            eth::Gas(gas)
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("contract call error: {0:?}")]
    ContractCall(#[from] alloy::contract::Error),

    #[error("can't simulate gas for an order for which the settlement contract is the owner")]
    SettlementContractIsOwner,
}
