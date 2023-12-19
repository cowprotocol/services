use {
    crate::{
        domain::{dex, eth},
        infra::blockchain,
    },
    contracts::ethcontract::{self, web3},
    ethereum_types::{Address, U256},
    ethrpc::extensions::EthExt,
    std::collections::HashMap,
};

/// A DEX swap simulator.
#[derive(Debug, Clone)]
pub struct Simulator {
    web3: ethrpc::Web3,
    settlement: eth::ContractAddress,
    authenticator: eth::ContractAddress,
}

impl Simulator {
    /// Create a new simulator for computing DEX swap gas usage.
    pub fn new(
        url: &reqwest::Url,
        settlement: eth::ContractAddress,
        authenticator: eth::ContractAddress,
    ) -> Self {
        Self {
            web3: blockchain::rpc(url),
            settlement,
            authenticator,
        }
    }

    /// Simulate the gas needed by a single order DEX swap.
    ///
    /// This will return a `None` if the gas simulation is unavailable.
    pub async fn gas(&self, owner: Address, swap: &dex::Swap) -> Result<eth::Gas, Error> {
        let swapper = contracts::support::Swapper::at(&self.web3, owner);
        let tx = swapper
            .methods()
            .swap(
                self.settlement.0,
                (swap.input.token.0, swap.input.amount),
                (swap.output.token.0, swap.output.amount),
                (swap.allowance.spender.0, swap.allowance.amount.get()),
                (
                    swap.call.to.0,
                    U256::zero(),
                    ethcontract::Bytes(swap.call.calldata.clone()),
                ),
            )
            .tx;

        let call = web3::types::CallRequest {
            to: tx.to,
            data: tx.data,
            ..Default::default()
        };

        let code = |contract: &contracts::ethcontract::Contract| {
            contract
                .deployed_bytecode
                .to_bytes()
                .expect("contract bytecode is available")
        };
        let overrides = HashMap::<_, _>::from_iter([
            // Setup up our trader code that actually executes the settlement
            (
                swapper.address(),
                ethrpc::extensions::StateOverride {
                    code: Some(code(contracts::support::Swapper::raw_contract())),
                    ..Default::default()
                },
            ),
            // Override the CoW protocol solver authenticator with one that
            // allows any address to solve
            (
                self.authenticator.0,
                ethrpc::extensions::StateOverride {
                    code: Some(code(contracts::support::AnyoneAuthenticator::raw_contract())),
                    ..Default::default()
                },
            ),
        ]);
        tracing::debug!(?call, "simulate swap gas usage");

        let return_data = self
            .web3
            .eth()
            .call_with_state_overrides(call, web3::types::BlockNumber::Latest.into(), overrides)
            .await?
            .0;

        let gas = {
            if return_data.len() != 32 {
                return Err(Error::InvalidReturnData);
            }

            U256::from_big_endian(&return_data)
        };

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
#[error("error initializing simulator: {0}")]
pub struct InitializationError(#[from] ethcontract::errors::MethodError);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),

    #[error("invalid return data")]
    InvalidReturnData,
}
