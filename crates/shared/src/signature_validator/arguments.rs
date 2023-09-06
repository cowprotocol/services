use {
    super::{SignatureValidating, SimulationSignatureValidator, Web3SignatureValidator},
    ethcontract::H160,
    ethrpc::Web3,
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
};

/// Arguments related to the token owner finder.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// The ERC-1271 signature validation strategy to use.
    #[clap(long, env, default_value = "web3", value_enum)]
    pub eip1271_signature_validator: Strategy,
}

/// Support token owner finding strategies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Strategy {
    /// Use basic Ethereum RPC requests to simulate `isValidSignature` calls.
    ///
    /// Note that this strategy does not properly support signatures for orders
    /// with custom interactions.
    Web3,

    /// Use code simulation techniques to simulate `isValidSignature` calls.
    ///
    /// This strategy fully supports signature verification for orders with
    /// custom interactions.
    Simulation,
}

/// Contracts required for balance simulation.
pub struct Contracts {
    pub chain_id: u64,
    pub settlement: H160,
    pub vault_relayer: H160,
}

impl Arguments {
    pub fn validator(&self, contracts: Contracts, web3: Web3) -> Arc<dyn SignatureValidating> {
        match self.eip1271_signature_validator {
            Strategy::Web3 => Arc::new(Web3SignatureValidator::new(web3)),
            Strategy::Simulation => Arc::new(SimulationSignatureValidator::new(
                web3,
                contracts.settlement,
                contracts.vault_relayer,
            )),
        }
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(
            f,
            "eip1271_signature_validator: {:?}",
            self.eip1271_signature_validator
        )?;

        Ok(())
    }
}
