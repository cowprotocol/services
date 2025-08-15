use {
    alloy::sol_types::{SolType, sol_data},
    ethcontract::{Bytes, contract::MethodBuilder, dyns::DynTransport},
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    model::{
        interaction::InteractionData,
        order::{Order, SellTokenSource},
    },
    primitive_types::{H160, U256},
    std::sync::Arc,
    thiserror::Error,
};

mod cached;
mod simulation;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Query {
    pub owner: H160,
    pub token: H160,
    pub source: SellTokenSource,
    pub interactions: Vec<InteractionData>,
}

impl Query {
    pub fn from_order(o: &Order) -> Self {
        Self {
            owner: o.metadata.owner,
            token: o.data.sell_token,
            source: o.data.sell_token_balance,
            interactions: o.interactions.pre.clone(),
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

#[mockall::automock]
#[async_trait::async_trait]
pub trait BalanceFetching: Send + Sync {
    // Returns the balance available to the allowance manager for the given owner
    // and token taking both balance as well as "allowance" into account.
    async fn get_balances(&self, queries: &[Query]) -> Vec<anyhow::Result<U256>>;

    // Check that the settlement contract can make use of this user's token balance.
    // This check could fail if the user does not have enough balance, has not
    // given the allowance to the allowance manager or if the token does not
    // allow freely transferring amounts around for for example if it is paused
    // or takes a fee on transfer. If the node supports the trace_callMany we
    // can perform more extensive tests.
    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError>;
}

/// Contracts required for balance simulation.
pub struct Contracts {
    pub settlement: contracts::GPv2Settlement,
    pub balances: contracts::support::Balances,
    pub vault_relayer: H160,
    pub vault: Option<H160>,
}

/// Create the default [`BalanceFetching`] instance.
pub fn fetcher(web3: &Web3, contracts: Contracts) -> Arc<dyn BalanceFetching> {
    Arc::new(simulation::Balances::new(
        web3,
        contracts.settlement,
        contracts.balances,
        contracts.vault_relayer,
        contracts.vault,
    ))
}

/// Create a cached [`BalanceFetching`] instance.
pub fn cached(
    web3: &Web3,
    contracts: Contracts,
    blocks: CurrentBlockWatcher,
) -> Arc<dyn BalanceFetching> {
    let cached = Arc::new(cached::Balances::new(fetcher(web3, contracts)));
    cached.spawn_background_task(blocks);
    cached
}

#[async_trait::async_trait]
pub trait BalanceSimulating: Send + Sync {
    fn settlement(&self) -> &contracts::GPv2Settlement;
    fn vault_relayer(&self) -> H160;
    fn vault(&self) -> H160;
    fn balances(&self) -> &contracts::support::Balances;

    async fn add_access_lists(
        &self,
        delegate_call: &mut MethodBuilder<DynTransport, Bytes<Vec<u8>>>,
    );

    async fn simulate(
        &self,
        owner: H160,
        token: H160,
        source: SellTokenSource,
        interactions: Vec<InteractionData>,
        amount: Option<U256>,
        disable_access_lists: bool,
    ) -> Result<Simulation, SimulationError> {
        // We simulate the balances from the Settlement contract's context. This
        // allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual VaultRelayer transfers that would happen as part of a
        //    settlement
        //
        // This allows us to end up with very accurate balance simulations.
        let balance_call = self.balances().balance(
            (
                self.settlement().address(),
                self.vault_relayer(),
                self.vault(),
            ),
            owner,
            token,
            amount.unwrap_or_default(),
            Bytes(source.as_bytes()),
            interactions
                .iter()
                .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                .collect(),
        );

        let mut delegate_call = self
            .settlement()
            .simulate_delegatecall(
                self.balances().address(),
                Bytes(balance_call.tx.data.unwrap_or_default().0),
            )
            .from(crate::SIMULATION_ACCOUNT.clone());

        if !disable_access_lists {
            // Add the access lists to the delegate call if they are enabled
            // system-wide.
            self.add_access_lists(&mut delegate_call).await;
        }

        let response = delegate_call.call().await?;
        let (token_balance, allowance, effective_balance, can_transfer) =
            <(
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Uint<256>,
                sol_data::Bool,
            )>::abi_decode(&response.0)
            .map_err(|err| {
                tracing::error!(?err, "failed to decode balance response");
                web3::error::Error::Decoder("failed to decode balance response".to_string())
            })?;

        let simulation = Simulation {
            token_balance: U256::from_little_endian(&token_balance.as_le_bytes()),
            allowance: U256::from_little_endian(&allowance.as_le_bytes()),
            effective_balance: U256::from_little_endian(&effective_balance.as_le_bytes()),
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
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
}
