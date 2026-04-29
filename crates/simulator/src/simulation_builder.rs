use {
    crate::{
        encoding::{EncodedSettlement, WrapperCall},
        tenderly::dto::StateObject,
    },
    alloy_primitives::{Address, Bytes, TxKind, U256},
    alloy_provider::{DynProvider, Provider},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    alloy_transport::RpcError,
    anyhow::{Context, Result},
    balance_overrides::BalanceOverriding,
    ethrpc::block_stream::CurrentBlockWatcher,
    model::{
        DomainSeparator,
        interaction::InteractionData,
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
    pub(crate) current_block: CurrentBlockWatcher,
    pub(crate) tenderly: Option<Arc<dyn crate::tenderly::Api>>,
}

impl SettlementSimulator {
    pub async fn new(
        settlement: contracts::GPv2Settlement::Instance,
        flash_loan_router: Address,
        balance_overrides: Arc<dyn BalanceOverriding>,
        current_block: CurrentBlockWatcher,
        tenderly: Option<Arc<dyn crate::tenderly::Api>>,
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
            current_block,
            tenderly,
        })))
    }

    pub fn new_simulation_builder(&self) -> SimulationBuilder {
        SimulationBuilder {
            simulator: self.clone(),
            order: None,
            pre_interactions: vec![],
            main_interactions: vec![],
            post_interactions: vec![],
            wrapper: WrapperConfig::NoWrapper,
            prices: None,
            solver: None,
            auction_id: None,
            state_overrides: StateOverride::default(),
            fund_settlement_contract: false,
            block: Block::Latest,
        }
    }
}

/// Which block to simulate against.
pub enum Block {
    /// Use the current head block from the block stream, pinning the
    /// simulation to a concrete number at build time.
    Latest,
    Number(u64),
}

/// Assembles a GPv2 settlement call for simulation purposes.
///
/// Call [`SimulationBuilder::build`] when done to produce a [`SettlementCall`].
pub struct SimulationBuilder {
    pub(crate) order: Option<Order>,
    pub(crate) pre_interactions: Vec<InteractionData>,
    pub(crate) main_interactions: Vec<InteractionData>,
    pub(crate) post_interactions: Vec<InteractionData>,
    pub(crate) wrapper: WrapperConfig,
    pub(crate) prices: Option<Prices>,
    pub(crate) solver: Option<Solver>,
    pub(crate) auction_id: Option<i64>,
    pub(crate) state_overrides: StateOverride,
    pub(crate) simulator: SettlementSimulator,
    pub(crate) fund_settlement_contract: bool,
    pub(crate) block: Block,
}

impl SimulationBuilder {
    // TODO: support multiple orders to support use case of encoding solutions
    // in the driver and the trade verification (requires JIT orders)
    pub fn add_order(mut self, order: Order) -> Self {
        self.order = Some(order);
        self
    }

    pub fn with_pre_interactions(mut self, interactions: Vec<InteractionData>) -> Self {
        self.pre_interactions = interactions;
        self
    }

    pub fn with_main_interactions(mut self, interactions: Vec<InteractionData>) -> Self {
        self.main_interactions = interactions;
        self
    }

    pub fn with_post_interactions(mut self, interactions: Vec<InteractionData>) -> Self {
        self.post_interactions = interactions;
        self
    }

    pub fn with_wrapper(mut self, wrapper: WrapperConfig) -> Self {
        self.wrapper = wrapper;
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

    pub fn at_block(mut self, block: Block) -> Self {
        self.block = block;
        self
    }

    /// Override the settlement contract's buy token balance so it can pay out
    /// the order without any external liquidity. The required amount is derived
    /// from the order's executed amount and clearing prices at `build()` time.
    pub fn fund_settlement_contract_with_buy_tokens(mut self) -> Self {
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
    pub(crate) pre_interactions: Vec<InteractionData>,
    pub(crate) post_interactions: Vec<InteractionData>,
    pub(crate) executed_amount: ExecutionAmount,
}

/// Configuration for wrapping the settlement in a flashloan or custom wrapper
/// contract chain.
pub enum WrapperConfig {
    Flashloan(Vec<FlashloanRequest>),
    Custom(Vec<WrapperCall>),
    NoWrapper,
    /// Parse the app data JSON string and extract flashloan/wrapper
    /// configuration.
    FromAppData(String),
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

    pub fn with_pre_interactions(mut self, interactions: Vec<InteractionData>) -> Self {
        self.pre_interactions = interactions;
        self
    }

    pub fn with_post_interactions(mut self, interactions: Vec<InteractionData>) -> Self {
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
    pub block: u64,
}

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Debug)]
pub struct TenderlyReport {
    /// Full request object that can be used directly with the Tenderly API
    pub tenderly_request: crate::tenderly::dto::Request,
    /// Shared Tenderly simulation URL for debugging in the dashboard
    pub tenderly_url: Option<String>,
    /// Any error that might have been reported during order simulation
    pub error: Option<String>,
}

impl EthCallInputs {
    pub async fn simulate(self) -> Result<Bytes, RpcError<alloy_transport::TransportErrorKind>> {
        self.simulator
            .0
            .provider
            .clone()
            .call(self.request)
            .overrides(self.state_overrides)
            .block(self.block.into())
            .await
    }

    pub async fn simulate_with_tenderly_report(self) -> Result<TenderlyReport, anyhow::Error> {
        // TODO: error handling
        let tenderly_request = self.to_tenderly_request().unwrap();
        let tenderly_url = match &self.simulator.0.tenderly {
            Some(api) => Some(
                api.simulate_and_share(tenderly_request.clone())
                    .await
                    .context("tenderly failed")?,
            ),
            None => None,
        };
        let simulation_result = self.simulate().await;

        Ok(TenderlyReport {
            tenderly_request,
            tenderly_url,
            error: match simulation_result {
                Ok(_) => None,
                Err(err) => Some(err.to_string()),
            },
        })
    }

    /// Converts the simulation into a request that can be simulated with
    /// tenderly.
    pub fn to_tenderly_request(&self) -> Result<crate::tenderly::dto::Request, ConversionError> {
        Ok(crate::tenderly::dto::Request {
            // By default tenderly simulates calls at the start of the block. So if we simulate
            // something when the latest block is `n` we need to tell tenderly to simulate at
            // block `n+1` to still have all of block n's txs happen before our simulation runs.
            block_number: Some(self.block + 1),
            network_id: self.simulator.0.chain_id.to_string(),
            from: self.request.from.unwrap_or_default(),
            // TODO: error handling
            to: match &self.request.to.ok_or(ConversionError::MissingTo)? {
                TxKind::Create => Default::default(),
                TxKind::Call(to) => *to,
            },
            input: self
                .request
                .input
                .input
                .as_ref()
                .map(|bytes| bytes.to_vec())
                .unwrap_or_default(),
            gas: self.request.gas,
            gas_price: None, // use tenderly default for now
            value: self.request.value,
            simulation_type: Some(crate::tenderly::dto::SimulationType::Full),
            state_objects: Some(
                self.state_overrides
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            *key,
                            StateObject::try_from(value.clone())
                                .map_err(|_| ConversionError::StateOverrides)?,
                        ))
                    })
                    .collect::<Result<_, ConversionError>>()?,
            ),
            access_list: self.request.access_list.as_ref().map(Into::into),
            save: Some(true),
            save_if_fails: Some(true),
            ..Default::default()
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("simulation does not have a target")]
    MissingTo,
    #[error("could not convert state overrides")]
    StateOverrides,
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
    #[error("failed to parse app data: {0}")]
    AppDataParse(#[source] serde_json::Error),
    #[error("both wrappers and flashloans cannot be encoded in the same settlement")]
    FlashloanWrappersIncompatible,
}
