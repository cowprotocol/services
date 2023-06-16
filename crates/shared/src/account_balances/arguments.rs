use {
    super::{BalanceFetching, CachingBalanceFetcher, SimulationBalanceFetcher, Web3BalanceFetcher},
    crate::{
        arguments::{display_option, CodeSimulatorKind},
        code_simulation::{CodeSimulating, TenderlyCodeSimulator, Web3ThenTenderly},
        current_block::CurrentBlockStream,
        ethrpc::Web3,
        tenderly_api::TenderlyApi,
    },
    ethcontract::H160,
    std::{
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
};

/// Arguments related to the token owner finder.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// The account balance simulation strategy to use.
    #[clap(long, env, default_value = "web3", value_enum)]
    pub account_balances: Strategy,

    /// The code simulation implementation to use. Can be one of `Web3`,
    /// `Tenderly` or `Web3ThenTenderly`.
    #[clap(long, env, value_enum)]
    pub account_balances_simulator: Option<CodeSimulatorKind>,
}

/// Support token owner finding strategies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Strategy {
    /// Use basic Ethereum RPC requests to query balances and allowances.
    ///
    /// Note that this strategy does not properly support fetching balances for
    /// orders with custom interactions.
    Web3,

    /// Use code simulation techniques to query balances and allowances.
    ///
    /// This strategy fully supports fetching balances for orders with custom
    /// interactions.
    Simulation,
}

/// Contracts required for balance simulation.
pub struct Contracts {
    pub chain_id: u64,
    pub settlement: H160,
    pub vault_relayer: H160,
    pub vault: Option<H160>,
}

impl Arguments {
    pub fn fetcher(
        &self,
        contracts: Contracts,
        web3: Web3,
        simulation_web3: Option<Web3>,
        tenderly: Option<Arc<dyn TenderlyApi>>,
    ) -> Arc<dyn BalanceFetching> {
        match self.account_balances {
            Strategy::Web3 => Arc::new(Web3BalanceFetcher::new(
                web3,
                contracts.vault,
                contracts.vault_relayer,
                contracts.settlement,
            )),
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
                    .account_balances_simulator
                    .expect("account balances simulator not configured")
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

                Arc::new(SimulationBalanceFetcher::new(
                    simulator,
                    contracts.settlement,
                    contracts.vault_relayer,
                    contracts.vault,
                ))
            }
        }
    }

    pub fn cached(
        &self,
        contracts: Contracts,
        web3: Web3,
        simulation_web3: Option<Web3>,
        tenderly: Option<Arc<dyn TenderlyApi>>,
        blocks: CurrentBlockStream,
    ) -> Arc<CachingBalanceFetcher> {
        let cached = Arc::new(CachingBalanceFetcher::new(self.fetcher(
            contracts,
            web3,
            simulation_web3,
            tenderly,
        )));
        cached.spawn_background_task(blocks);
        cached
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "account_balances: {:?}", self.account_balances)?;
        display_option(
            f,
            "account_balances_simulator",
            &self
                .account_balances_simulator
                .as_ref()
                .map(|value| format!("{value:?}")),
        )?;

        Ok(())
    }
}
