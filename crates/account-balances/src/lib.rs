use {
    alloy_primitives::{Address, U256},
    alloy_rpc_types::state::StateOverride,
    alloy_sol_types::{SolCall, SolType, sol_data},
    balance_overrides::{BalanceOverrideRequest, StateOverriding},
    contracts::{GPv2Settlement, support::Balances},
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    model::{
        interaction::InteractionData,
        order::{Order, SellTokenSource},
    },
    std::sync::{Arc, LazyLock},
};

mod cached;
mod simulation;

pub type BlockNumber = u64;

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
    TransferFailed(Vec<u8>),
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
    async fn get_balances(
        &self,
        queries: &[Query],
        block_number: Option<BlockNumber>,
    ) -> Vec<anyhow::Result<U256>>;

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

#[derive(Clone, Debug, Default)]
pub(crate) struct SimulateParams {
    amount: Option<U256>,
    balance_override: Option<BalanceOverrideRequest>,
    block_number: Option<BlockNumber>,
}

impl SimulateParams {
    pub(crate) fn new(
        amount: Option<U256>,
        balance_override: Option<BalanceOverrideRequest>,
        block_number: Option<BlockNumber>,
    ) -> Self {
        Self {
            amount,
            balance_override,
            block_number,
        }
    }
}

#[derive(Clone)]
pub struct BalanceSimulator {
    settlement: GPv2Settlement::Instance,
    balances: Balances::Instance,
    vault_relayer: Address,
    vault: Address,
    balance_overrider: Arc<dyn StateOverriding>,
}

impl BalanceSimulator {
    pub fn new(
        settlement: GPv2Settlement::Instance,
        balances: Balances::Instance,
        vault_relayer: Address,
        vault: Option<Address>,
        balance_overrider: Arc<dyn StateOverriding>,
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

    fn block_id_from_number(block_number: Option<BlockNumber>) -> alloy_rpc_types::BlockId {
        block_number
            .map(alloy_rpc_types::BlockId::number)
            .unwrap_or_else(alloy_rpc_types::BlockId::latest)
    }

    pub(crate) async fn simulate(
        &self,
        owner: Address,
        token: Address,
        source: SellTokenSource,
        interactions: &[InteractionData],
        params: SimulateParams,
    ) -> Result<Simulation, SimulationError> {
        let amount = params.amount;
        let balance_override = params.balance_override;
        let block_number = params.block_number;

        let overrides: StateOverride = match balance_override {
            Some(overrides) => self
                .balance_overrider
                .balance_override(overrides)
                .await
                .into_iter()
                .collect(),
            None => Default::default(),
        };
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

        let block_id = Self::block_id_from_number(block_number);

        let call_builder = self
            .settlement
            .simulateDelegatecall(*self.balances.address(), balance_call.abi_encode().into())
            .with_cloned_provider()
            .state(overrides)
            .from(*SIMULATION_ACCOUNT)
            .block(block_id);

        let response = call_builder.call().await?;

        let (token_balance, allowance, effective_balance, can_transfer, transfer_revert_reason) =
            <(
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Bool,
                sol_data::Bytes,
            )>::abi_decode_params(&response.0)
            .map_err(|err| {
                tracing::error!(?err, "failed to decode balance response");
                alloy_contract::Error::AbiError(alloy_dyn_abi::Error::SolTypes(err))
            })?;

        let simulation = Simulation {
            token_balance: U256::from_le_slice(&token_balance.as_le_bytes()),
            allowance: U256::from_le_slice(&allowance.as_le_bytes()),
            effective_balance: U256::from_le_slice(&effective_balance.as_le_bytes()),
            can_transfer,
            transfer_revert_reason: transfer_revert_reason.to_vec(),
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
    pub transfer_revert_reason: Vec<u8>,
}

#[derive(Debug)]
pub enum SimulationError {
    Method(alloy_contract::Error),
}

impl std::fmt::Display for SimulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Method(err) => write!(f, "method error: {err:?}"),
        }
    }
}

impl std::error::Error for SimulationError {}

impl From<alloy_contract::Error> for SimulationError {
    fn from(err: alloy_contract::Error) -> Self {
        Self::Method(err)
    }
}

// ZKSync-based chains don't use the default 0x0 account when `tx.from` is not
// specified, so we need to use a random account when sending a simulation tx.
static SIMULATION_ACCOUNT: LazyLock<Address> = LazyLock::new(|| Address::random());
