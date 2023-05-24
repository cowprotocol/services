use {
    crate::{
        boundary::ethrpc::{self, extensions::EthExt},
        domain::{dex, eth},
    },
    contracts::ethcontract::{self, dyns::DynWeb3, transport::DynTransport, web3},
    ethereum_types::{Address, U256},
    std::collections::HashMap,
};

/// A DEX swap simulator.
#[derive(Debug, Clone)]
pub struct Simulator {
    web3: DynWeb3,
    authenticator: eth::ContractAddress,
}

impl Simulator {
    /// Create a new simulator for computing DEX swap gas usage.
    pub fn new(url: &reqwest::Url, authenticator: eth::ContractAddress) -> Self {
        let web3 = DynWeb3::new(DynTransport::new(
            web3::transports::Http::new(url.as_str()).expect("url is valid"),
        ));

        Self {
            web3,
            authenticator,
        }
    }

    /// Simulate the gas needed by a single order DEX swap.
    pub async fn gas(&self, trader: Address, swap: &dex::Swap) -> Result<eth::Gas, Error> {
        let trader = contracts::support::Trader::at(&self.web3, trader);
        let tx = trader
            .methods()
            .swap(
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
                trader.address(),
                ethrpc::extensions::StateOverride {
                    code: Some(code(contracts::support::Trader::raw_contract())),
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

        let return_data = self
            .web3
            .eth()
            .call_with_state_overrides(call, web3::types::BlockNumber::Latest.into(), overrides)
            .await?
            .0;

        if return_data.len() != 32 {
            return Err(Error::InvalidReturnData);
        }
        let gas = U256::from_big_endian(&return_data);

        Ok(eth::Gas(gas))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),

    #[error("invalid return data")]
    InvalidReturnData,
}
