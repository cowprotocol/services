use {
    super::{SignatureValidating, SimulationSignatureValidator, Web3SignatureValidator},
    crate::{
        arguments::{display_option, CodeSimulatorKind},
        code_simulation::{CodeSimulating, TenderlyCodeSimulator, Web3ThenTenderly},
        tenderly_api::TenderlyApi,
    },
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

    /// The code simulation implementation to use. Can be one of `Web3`,
    /// `Tenderly` or `Web3ThenTenderly`.
    #[clap(long, env, value_enum)]
    pub eip1271_signature_validator_simulator: Option<CodeSimulatorKind>,
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
    pub fn validator(
        &self,
        contracts: Contracts,
        web3: Web3,
        simulation_web3: Option<Web3>,
        tenderly: Option<Arc<dyn TenderlyApi>>,
    ) -> Arc<dyn SignatureValidating> {
        match self.eip1271_signature_validator {
            Strategy::Web3 => Arc::new(Web3SignatureValidator::new(web3)),
            Strategy::Simulation => {
                let web3_simulator =
                    move || simulation_web3.expect("simulation web3 not configured");
                let tenderly_simulator = move || {
                    TenderlyCodeSimulator::new(
                        tenderly.expect("tenderly api not configured"),
                        contracts.chain_id,
                    )
                };

                let simulator = match self
                    .eip1271_signature_validator_simulator
                    .expect("ERC-1271 signature validator simulator not configured")
                {
                    CodeSimulatorKind::Web3 => {
                        Arc::new(web3_simulator()) as Arc<dyn CodeSimulating>
                    }
                    CodeSimulatorKind::Tenderly => Arc::new(tenderly_simulator()),
                    CodeSimulatorKind::Web3ThenTenderly => Arc::new(Web3ThenTenderly::new(
                        web3_simulator(),
                        tenderly_simulator(),
                    )),
                };

                Arc::new(SimulationSignatureValidator::new(
                    simulator,
                    contracts.settlement,
                    contracts.vault_relayer,
                ))
            }
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
        display_option(
            f,
            "eip1271_signature_validator_simulator",
            &self
                .eip1271_signature_validator_simulator
                .as_ref()
                .map(|value| format!("{value:?}")),
        )?;

        Ok(())
    }
}
