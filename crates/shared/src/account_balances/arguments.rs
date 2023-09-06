use {
    super::{BalanceFetching, CachingBalanceFetcher, SimulationBalanceFetcher, Web3BalanceFetcher},
    ethcontract::H160,
    ethrpc::{current_block::CurrentBlockStream, Web3},
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

    /// Whether or not to optimistically treat account balance queries with
    /// pre-interactions as if sufficient token balance and allowance is always
    /// available. Useful for partially supporting pre-interactions in
    /// environments where the required simulation infrastructure is not
    /// available (such as in E2E tests).
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub account_balances_optimistic_pre_interaction_handling: bool,
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
    pub fn fetcher(&self, contracts: Contracts, web3: Web3) -> Arc<dyn BalanceFetching> {
        match self.account_balances {
            Strategy::Web3 => Arc::new(Web3BalanceFetcher::new(
                web3,
                contracts.vault,
                contracts.vault_relayer,
                contracts.settlement,
                self.account_balances_optimistic_pre_interaction_handling,
            )),
            Strategy::Simulation => Arc::new(SimulationBalanceFetcher::new(
                web3,
                contracts.settlement,
                contracts.vault_relayer,
                contracts.vault,
            )),
        }
    }

    pub fn cached(
        &self,
        contracts: Contracts,
        web3: Web3,
        blocks: CurrentBlockStream,
    ) -> Arc<CachingBalanceFetcher> {
        let cached = Arc::new(CachingBalanceFetcher::new(self.fetcher(contracts, web3)));
        cached.spawn_background_task(blocks);
        cached
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "account_balances: {:?}", self.account_balances)?;

        Ok(())
    }
}
