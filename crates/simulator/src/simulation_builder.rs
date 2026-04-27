use {
    crate::encoding::{EncodedSettlement, Interaction, WrapperCall},
    alloy_eips::BlockId,
    alloy_network::Ethereum,
    alloy_primitives::{Address, Bytes, U256},
    alloy_provider::{DynProvider, EthCall, Provider},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    anyhow::Result,
    balance_overrides::BalanceOverriding,
    model::{
        DomainSeparator,
        order::OrderData,
        signature::{Signature, SigningScheme},
    },
    std::sync::Arc,
};

/// Holds the settlement contract and its authenticator address, and acts as a
/// factory for [`SimulationBuilder`] instances that are pre-configured with
/// these values.
#[derive(Clone)]
pub struct SettlementSimulator(pub(crate) Arc<Inner>);

pub(crate) struct Inner {
    pub(crate) settlement: contracts::GPv2Settlement::Instance,
    pub(crate) authenticator: Address,
    pub(crate) flash_loan_router: Address,
    pub(crate) balance_overrides: Arc<dyn BalanceOverriding>,
    pub(crate) provider: DynProvider,
    pub(crate) domain_separator: DomainSeparator,
    pub(crate) chain_id: u64,
}

impl SettlementSimulator {
    pub async fn new(
        settlement: contracts::GPv2Settlement::Instance,
        flash_loan_router: Address,
        balance_overrides: Arc<dyn BalanceOverriding>,
    ) -> Result<Self> {
        let authenticator = Address(settlement.authenticator().call().await?.0);
        let domain_separator = DomainSeparator(settlement.domainSeparator().call().await?.0);
        let provider = settlement.provider().clone();
        let chain_id = provider.get_chain_id().await?;
        Ok(Self(Arc::new(Inner {
            settlement,
            authenticator,
            flash_loan_router,
            balance_overrides,
            provider,
            domain_separator,
            chain_id,
        })))
    }

    pub fn new_simulation_builder(&self) -> SimulationBuilder {
        SimulationBuilder {
            simulator: self.clone(),
            order: None,
            pre_interactions: vec![],
            main_interactions: vec![],
            post_interactions: vec![],
            wrapper: None,
            prices: None,
            solver: None,
            auction_id: None,
            state_overrides: StateOverride::default(),
            fund_settlement_contract: false,
            block: BlockId::latest(),
        }
    }
}

/// Assembles a GPv2 settlement call for simulation purposes.
///
/// Call [`SimulationBuilder::build`] when done to produce a [`SettlementCall`].
pub struct SimulationBuilder {
    pub(crate) order: Option<Order>,
    pub(crate) pre_interactions: Vec<Interaction>,
    pub(crate) main_interactions: Vec<Interaction>,
    pub(crate) post_interactions: Vec<Interaction>,
    pub(crate) wrapper: Option<WrapperConfig>,
    pub(crate) prices: Option<Prices>,
    pub(crate) solver: Option<Solver>,
    pub(crate) auction_id: Option<i64>,
    pub(crate) state_overrides: StateOverride,
    pub(crate) simulator: SettlementSimulator,
    pub(crate) fund_settlement_contract: bool,
    pub(crate) block: BlockId,
}

impl SimulationBuilder {
    // TODO: support multiple orders to support use case of encoding solutions
    // in the driver and the trade verification (requires JIT orders)
    pub fn add_order(mut self, order: Order) -> Self {
        self.order = Some(order);
        self
    }

    pub fn with_pre_interactions(mut self, interactions: Vec<Interaction>) -> Self {
        self.pre_interactions = interactions;
        self
    }

    pub fn with_main_interactions(mut self, interactions: Vec<Interaction>) -> Self {
        self.main_interactions = interactions;
        self
    }

    pub fn with_post_interactions(mut self, interactions: Vec<Interaction>) -> Self {
        self.post_interactions = interactions;
        self
    }

    pub fn with_wrapper(mut self, wrapper: WrapperConfig) -> Self {
        self.wrapper = Some(wrapper);
        self
    }

    pub fn with_prices(mut self, prices: Prices) -> Self {
        self.prices = Some(prices);
        self
    }

    pub fn from_solver(mut self, solver: Solver) -> Self {
        self.solver = Some(solver);
        self
    }

    pub fn with_auction_id(mut self, id: i64) -> Self {
        self.auction_id = Some(id);
        self
    }

    pub fn state_override(
        mut self,
        address: Address,
        account_override: impl Into<AccountOverride>,
    ) -> Self {
        self.state_overrides
            .insert(address, account_override.into());
        self
    }

    pub fn at_block(mut self, block: BlockId) -> Self {
        self.block = block;
        self
    }

    /// Override the settlement contract's buy token balance so it can pay out
    /// the order without any external liquidity. The required amount is derived
    /// from the order's executed amount and clearing prices at `build()` time.
    pub fn fund_settlement_contract(mut self) -> Self {
        self.fund_settlement_contract = true;
        self
    }

    /// Finishes the simulation struct based on the configuration thus far.
    pub async fn build(self) -> Result<EthCallInputs, BuildError> {
        self.build_with_modifications(|_| {}).await
    }

    /// Same as `build()` but allows the caller to alter the simulation
    /// before it gets finalized. This should only be used for very specific
    /// setups.
    pub async fn build_with_modifications(
        self,
        customize: impl FnOnce(&mut EncodedSettlement),
    ) -> Result<EthCallInputs, BuildError> {
        // Forward to a helper function to split the boring repetitive builder
        // code from the non-trivial code that actually does the encoding.
        crate::simulation_encoding::encode(self, customize).await
    }
}

pub enum Solver {
    /// Simulation assumes this is an actual solver so no state overrides will
    /// be applied to allow list it explicitly.
    /// If you need a very specific solver setup for your simulation consider
    /// using this and explicitly add the necessary state overrides yourself
    /// with `Simulation::build_with_modifications()`.
    Real(Address),
    /// A fake solver for simulation. Uses the provided address or generates a
    /// random one. The simulation builder will automatically set the required
    /// state overrides to give it enough ETH and allow list it as a solver.
    Fake(Option<Address>),
}

/// How clearing prices are determined for the encoded settlement.
pub enum Prices {
    /// Derive clearing prices directly from the order's limit price.
    ///
    /// Sets `price[sell_token] = buy_amount` and `price[buy_token] =
    /// sell_amount`, exactly satisfying the order's limit with no surplus.
    /// This should NOT be used when encoding solutions you actually want
    /// to submit.
    Limit,
    // TODO: check how this can be made nicer.
    /// Explicit token list and matching clearing prices.
    Explicit {
        tokens: Vec<Address>,
        clearing_prices: Vec<U256>,
    },
}

/// How much of an order should be filled during simulation.
pub enum ExecutionAmount {
    /// Fill the full order amount (sell_amount for sell orders, buy_amount for
    /// buy orders), ignoring any on-chain filled state.
    Full,
    /// Fill whatever is still remaining on-chain (queries the settlement
    /// contract for the already-filled amount and subtracts it). Falls back to
    /// the full amount if the query fails.
    Remaining,
    /// Use an explicit fill amount.
    Explicit(U256),
}

/// A simulator-specific order that bundles the data needed to encode a trade.
///
/// Construct with [`Order::new`] and add optional fields via the builder
/// methods. Defaults to an EIP-1271 signature (pairs with [`FakeUser`] for
/// simulations that need to bypass signature verification).
pub struct Order {
    pub(crate) data: OrderData,
    pub(crate) owner: Address,
    pub(crate) signature: Signature,
    pub(crate) pre_interactions: Vec<Interaction>,
    pub(crate) post_interactions: Vec<Interaction>,
    pub(crate) executed_amount: ExecutionAmount,
}

/// Configuration for wrapping the settlement in a flashloan or custom wrapper
/// contract chain.
pub enum WrapperConfig {
    Flashloan(Vec<FlashloanRequest>),
    Custom(Vec<WrapperCall>),
}

pub struct FlashloanRequest {
    pub amount: U256,
    pub borrower: Address,
    pub lender: Address,
    pub token: Address,
}

impl Order {
    pub fn new(data: OrderData) -> Self {
        Self {
            data,
            owner: Address::ZERO,
            signature: Signature::default_with(SigningScheme::Eip1271),
            pre_interactions: vec![],
            post_interactions: vec![],
            executed_amount: ExecutionAmount::Remaining,
        }
    }

    pub fn with_signature(mut self, owner: Address, signature: Signature) -> Self {
        self.owner = owner;
        self.signature = signature;
        self
    }

    pub fn with_pre_interactions(mut self, interactions: Vec<Interaction>) -> Self {
        self.pre_interactions = interactions;
        self
    }

    pub fn with_post_interactions(mut self, interactions: Vec<Interaction>) -> Self {
        self.post_interactions = interactions;
        self
    }

    pub fn with_executed_amount(mut self, amount: ExecutionAmount) -> Self {
        self.executed_amount = amount;
        self
    }
}

/// The output of [`SimulationBuilder::build`]: a transaction request and state
/// overrides ready to be passed to an alloy provider for simulation.
pub struct EthCallInputs {
    pub request: TransactionRequest,
    pub state_overrides: StateOverride,
    pub simulator: SettlementSimulator,
    pub block: BlockId,
}

impl EthCallInputs {
    /// Prepares an `eth_call` with the transaction request, state overrides,
    /// and block already applied. The call is not sent — callers can chain
    /// additional builder methods before awaiting.
    pub fn simulate(self) -> EthCall<Ethereum, Bytes> {
        self.simulator
            .0
            .provider
            .clone()
            .call(self.request)
            .overrides(self.state_overrides)
            .block(self.block)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("no order was added")]
    NoOrder,
    #[error("no solver was set")]
    NoSolver,
    #[error("sell token not found in token list")]
    MissingSellToken,
    #[error("buy token not found in token list")]
    MissingBuyToken,
    #[error("could not override token balances to fund settlement contract")]
    FailedToOverrideBalances,
    #[error("no strategy to compute the price vector was chosen")]
    NoPriceEncoding,
    #[error("failed to query filled amount from settlement contract: {0}")]
    FilledAmountQuery(#[source] anyhow::Error),
}
