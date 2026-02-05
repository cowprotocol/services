use {
    crate::price_estimation::trade_verifier::balance_overrides::{
        BalanceOverrideRequest,
        BalanceOverriding,
    },
    alloy::{
        primitives::{Address, U256},
        rpc::types::state::StateOverride,
        sol_types::{SolCall, SolType, sol_data},
    },
    contracts::alloy::{GPv2Settlement, support::Balances},
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    model::{
        interaction::InteractionData,
        order::{Order, SellTokenSource},
    },
    std::sync::Arc,
    thiserror::Error,
};

mod cached;
mod simulation;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Query {
    pub owner: Address,
    pub token: Address,
    pub source: SellTokenSource,
    pub interactions: Vec<InteractionData>,
    pub balance_override: Option<BalanceOverrideRequest>,
}

impl Query {
    pub fn from_order(o: &Order) -> Self {
        Self {
            owner: o.metadata.owner,
            token: o.data.sell_token,
            source: o.data.sell_token_balance,
            interactions: o.interactions.pre.clone(),
            // TODO eventually delete together with the balance
            // checks in the autopilot
            balance_override: None,
        }
    }
}

#[derive(Debug)]
pub enum TransferSimulationError {
    InsufficientAllowance,
    InsufficientBalance,
    TransferFailed,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for TransferSimulationError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait::async_trait]
pub trait BalanceFetching: Send + Sync {
    // Returns the balance available to the allowance manager for the given owner
    // and token taking both balance as well as "allowance" into account.
    async fn get_balances(&self, queries: &[Query]) -> Vec<anyhow::Result<U256>>;

    // Check that the settlement contract can make use of this user's token balance.
    // This check could fail if the user does not have enough balance, has not
    // given the allowance to the allowance manager or if the token does not
    // allow freely transferring amounts around for example if it is paused
    // or takes a fee on transfer. If the node supports the trace_callMany we
    // can perform more extensive tests.
    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError>;
}

/// Create the default [`BalanceFetching`] instance.
pub fn fetcher(web3: &Web3, balance_simulator: BalanceSimulator) -> Arc<dyn BalanceFetching> {
    Arc::new(simulation::Balances::new(web3, balance_simulator))
}

/// Create a cached [`BalanceFetching`] instance.
pub fn cached(
    web3: &Web3,
    balance_simulator: BalanceSimulator,
    blocks: CurrentBlockWatcher,
) -> Arc<dyn BalanceFetching> {
    let cached = Arc::new(cached::Balances::new(fetcher(web3, balance_simulator)));
    cached.spawn_background_task(blocks);
    cached
}

#[derive(Clone)]
pub struct BalanceSimulator {
    settlement: GPv2Settlement::Instance,
    balances: Balances::Instance,
    vault_relayer: Address,
    vault: Address,
    balance_overrider: Arc<dyn BalanceOverriding>,
}

impl BalanceSimulator {
    pub fn new(
        settlement: GPv2Settlement::Instance,
        balances: Balances::Instance,
        vault_relayer: Address,
        vault: Option<Address>,
        balance_overrider: Arc<dyn BalanceOverriding>,
    ) -> Self {
        Self {
            settlement,
            vault_relayer,
            vault: vault.unwrap_or_default(),
            balances,
            balance_overrider,
        }
    }

    pub fn vault_relayer(&self) -> Address {
        self.vault_relayer
    }

    pub fn vault(&self) -> Address {
        self.vault
    }

    pub async fn simulate(
        &self,
        owner: Address,
        token: Address,
        source: SellTokenSource,
        interactions: &[InteractionData],
        amount: Option<U256>,
        balance_override: Option<BalanceOverrideRequest>,
    ) -> Result<Simulation, SimulationError> {
        let overrides: StateOverride = match balance_override {
            Some(overrides) => self
                .balance_overrider
                .state_override(overrides)
                .await
                .into_iter()
                .collect(),
            None => Default::default(),
        };
        // We simulate the balances from the Settlement contract's context. This
        // allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual VaultRelayer transfers that would happen as part of a
        //    settlement
        //
        // This allows us to end up with very accurate balance simulations.
        let balance_call = Balances::Balances::balanceCall {
            contracts: Balances::Balances::Contracts {
                settlement: *self.settlement.address(),
                vaultRelayer: self.vault_relayer,
                vault: self.vault,
            },
            trader: owner,
            token,
            amount: amount.unwrap_or_default(),
            source: source.as_bytes().into(),
            interactions: interactions
                .iter()
                .map(|i| Balances::Balances::Interaction {
                    target: i.target,
                    value: i.value,
                    callData: i.call_data.clone().into(),
                })
                .collect(),
        };

        let response = self
            .settlement
            .simulateDelegatecall(*self.balances.address(), balance_call.abi_encode().into())
            .with_cloned_provider()
            .state(overrides)
            .from(*crate::SIMULATION_ACCOUNT)
            .call()
            .await?;

        let (token_balance, allowance, effective_balance, can_transfer) =
            <(
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Bool,
            )>::abi_decode(&response.0)
            .map_err(|err| {
                tracing::error!(?err, "failed to decode balance response");
                alloy::contract::Error::AbiError(alloy::dyn_abi::Error::SolTypes(err))
            })?;

        let simulation = Simulation {
            token_balance: U256::from_le_slice(&token_balance.as_le_bytes()),
            allowance: U256::from_le_slice(&allowance.as_le_bytes()),
            effective_balance: U256::from_le_slice(&effective_balance.as_le_bytes()),
            can_transfer,
        };

        tracing::trace!(
            ?owner,
            ?token,
            ?source,
            ?amount,
            ?interactions,
            ?simulation,
            "simulated balances"
        );
        Ok(simulation)
    }
}

#[derive(Debug)]
pub struct Simulation {
    pub token_balance: U256,
    pub allowance: U256,
    pub effective_balance: U256,
    pub can_transfer: bool,
}

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("method error: {0:?}")]
    Method(#[from] alloy::contract::Error),
}
