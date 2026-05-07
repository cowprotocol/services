use {
    crate::{encoding::WrapperCall, tenderly},
    alloy_primitives::{Address, B256, Bytes, TxKind, U256},
    alloy_provider::{DynProvider, Provider},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    alloy_sol_types::SolCall,
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
    pub(crate) hooks_trampoline: Address,
    pub(crate) native_token: Address,
    pub(crate) max_gas_limit: u64,
    pub(crate) balance_overrides: Arc<dyn BalanceOverriding>,
    pub(crate) provider: DynProvider,
    pub(crate) domain_separator: DomainSeparator,
    pub(crate) chain_id: u64,
    pub(crate) current_block: CurrentBlockWatcher,
    pub(crate) tenderly: Option<Arc<dyn tenderly::Api>>,
}

impl SettlementSimulator {
    #[expect(clippy::too_many_arguments)]
    pub async fn new(
        settlement: contracts::GPv2Settlement::Instance,
        flash_loan_router: Address,
        hooks_trampoline: Address,
        native_token: Address,
        max_gas_limit: u64,
        balance_overrides: Arc<dyn BalanceOverriding>,
        current_block: CurrentBlockWatcher,
        tenderly: Option<Arc<dyn tenderly::Api>>,
    ) -> Result<Self> {
        let authenticator = Address(settlement.authenticator().call().await?.0);
        let domain_separator = DomainSeparator(settlement.domainSeparator().call().await?.0);
        let provider = settlement.provider().clone();
        let chain_id = provider.get_chain_id().await?;
        Ok(Self(Arc::new(Inner {
            settlement,
            authenticator,
            flash_loan_router,
            hooks_trampoline,
            native_token,
            max_gas_limit,
            balance_overrides,
            provider,
            domain_separator,
            chain_id,
            current_block,
            tenderly,
        })))
    }

    pub fn native_token(&self) -> Address {
        self.0.native_token
    }

    pub fn max_gas_limit(&self) -> u64 {
        self.0.max_gas_limit
    }

    pub fn provider(&self) -> DynProvider {
        self.0.provider.clone()
    }

    pub fn settlement_address(&self) -> Address {
        *self.0.settlement.address()
    }

    pub fn authenticator_address(&self) -> Address {
        self.0.authenticator
    }

    pub fn tenderly(&self) -> Option<Arc<dyn tenderly::Api>> {
        self.0.tenderly.clone()
    }

    pub fn new_simulation_builder(&self) -> SimulationBuilder {
        SimulationBuilder {
            simulator: self.clone(),
            orders: vec![],
            pre_interactions: vec![],
            main_interactions: vec![],
            post_interactions: vec![],
            wrapper: WrapperConfig::NoWrapper,
            solver: None,
            auction_id: None,
            account_override_requests: vec![],
            provide_buy_tokens: false,
            block: Block::Latest,
        }
    }

    pub fn domain_separator(&self) -> DomainSeparator {
        self.0.domain_separator
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
    pub(crate) orders: Vec<Order>,
    pub(crate) pre_interactions: Vec<InteractionData>,
    pub(crate) main_interactions: Vec<InteractionData>,
    pub(crate) post_interactions: Vec<InteractionData>,
    pub(crate) wrapper: WrapperConfig,
    pub(crate) solver: Option<Solver>,
    pub(crate) auction_id: Option<i64>,
    pub(crate) simulator: SettlementSimulator,
    pub(crate) account_override_requests: Vec<AccountOverrideRequest>,
    pub(crate) provide_buy_tokens: bool,
    pub(crate) block: Block,
}

impl SimulationBuilder {
    pub fn with_orders(mut self, orders: impl IntoIterator<Item = Order>) -> Self {
        self.orders = orders.into_iter().collect();
        self
    }

    pub fn with_pre_interactions(
        mut self,
        interactions: impl IntoIterator<Item = InteractionData>,
    ) -> Self {
        self.pre_interactions.extend(interactions);
        self
    }

    pub fn with_main_interactions(
        mut self,
        interactions: impl IntoIterator<Item = InteractionData>,
    ) -> Self {
        self.main_interactions.extend(interactions);
        self
    }

    pub fn with_post_interactions(
        mut self,
        interactions: impl IntoIterator<Item = InteractionData>,
    ) -> Self {
        self.post_interactions.extend(interactions);
        self
    }

    pub fn with_wrapper(mut self, wrapper: WrapperConfig) -> Self {
        self.wrapper = wrapper;
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

    pub fn at_block(mut self, block: Block) -> Self {
        self.block = block;
        self
    }

    /// Parses the app data JSON and configures the builder accordingly:
    /// - Pre/post hooks are encoded as interactions via the [`HooksTrampoline`]
    /// - Flashloan/wrapper fields set the [`WrapperConfig`].
    pub fn parameters_from_app_data(mut self, app_data: &str) -> Result<Self, BuildError> {
        let protocol = app_data::parse(app_data.as_bytes()).map_err(BuildError::AppDataParse)?;

        self.pre_interactions
            .extend(self.encode_hooks(&protocol.hooks.pre));
        self.post_interactions
            .extend(self.encode_hooks(&protocol.hooks.post));

        match (protocol.wrappers.is_empty(), protocol.flashloan) {
            (false, Some(_)) => return Err(BuildError::FlashloanWrappersIncompatible),
            (false, None) => {
                self.wrapper = WrapperConfig::Custom(
                    protocol
                        .wrappers
                        .into_iter()
                        .map(|w| WrapperCall {
                            address: w.address,
                            data: w.data.into(),
                        })
                        .collect(),
                );
            }
            (true, Some(flashloan)) => {
                self.wrapper = WrapperConfig::Flashloan(vec![FlashloanRequest {
                    amount: flashloan.amount,
                    borrower: flashloan.protocol_adapter,
                    lender: flashloan.liquidity_provider,
                    token: flashloan.token,
                }]);
            }
            (true, None) => {}
        }

        Ok(self)
    }

    /// Generates 1 interaction executing the given hooks via the trampoline
    /// contract since executing hooks directly from the settlement contract
    /// context would give them elevated privileges that put funds at risk.
    fn encode_hooks(&self, hooks: &[app_data::Hook]) -> Option<InteractionData> {
        if hooks.is_empty() {
            return None;
        }
        Some(InteractionData {
            target: self.simulator.0.hooks_trampoline,
            value: U256::ZERO,
            call_data: contracts::HooksTrampoline::HooksTrampoline::executeCall {
                hooks: hooks
                    .iter()
                    .map(|h| contracts::HooksTrampoline::HooksTrampoline::Hook {
                        target: h.target,
                        callData: Bytes::copy_from_slice(&h.call_data),
                        gasLimit: U256::from(h.gas_limit),
                    })
                    .collect(),
            }
            .abi_encode(),
        })
    }

    /// Instructs the builder to override the settlement contract's buy-token
    /// balances so it can pay out every order. The exact amounts are derived
    /// from the clearing prices and executed amounts once
    /// [`build`](Self::build) is called.
    pub fn provide_sufficient_buy_tokens(mut self) -> Self {
        self.provide_buy_tokens = true;
        self
    }

    /// Queues [`AccountOverrideRequest`]s to be resolved and applied during
    /// [`build`](Self::build). Multiple requests may target the same address
    /// and will be applied on a best-effort basis (failure to compute balance
    /// overrides or conflicting state overrides will get logged but do not
    /// lead to an error).
    pub fn with_overrides(
        mut self,
        requests: impl IntoIterator<Item = AccountOverrideRequest>,
    ) -> Self {
        self.account_override_requests.extend(requests);
        self
    }

    /// Finishes the simulation struct based on the configuration thus far.
    pub async fn build(self) -> Result<EthCallInputs, BuildError> {
        // Forward to a helper function to split the boring repetitive builder
        // code from the non-trivial code that actually does the encoding.
        crate::encoding::finish_simulation_builder(self).await
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

impl EthCallInputs {
    /// Runs the generated simulation using an `eth_call` and returns the
    /// response bytes if there are any.
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

    /// Same as [`EthCallInputs::simulate`] but also generates a tenderly
    /// request in case one wants to re-simulate with tenderly. If tenderly
    /// credentials are configured this even generates a shareable link for
    /// the simulation.
    pub async fn simulate_with_tenderly_report(self) -> Result<TenderlyReport, anyhow::Error> {
        let tenderly_request = self
            .to_tenderly_request()
            .context("failed to convert to tenderly request")?;

        let tenderly_url = if let Some(api) = &self.simulator.0.tenderly {
            api.simulate_and_share(tenderly_request.clone())
                .await
                .inspect_err(|err| tracing::warn!(?err, "failed to simulate via tenderly"))
                .ok()
        } else {
            None
        };

        Ok(TenderlyReport {
            tenderly_request,
            tenderly_url,
            error: self.simulate().await.err().map(|err| err.to_string()),
        })
    }

    /// Converts the simulation into a request that can be simulated with
    /// tenderly.
    pub fn to_tenderly_request(&self) -> Result<tenderly::dto::Request, ConversionError> {
        Ok(tenderly::dto::Request {
            // By default tenderly simulates calls at the start of the block. So if we simulate
            // something when the latest block is `n` we need to tell tenderly to simulate at
            // block `n+1` to still have all of block n's txs happen before our simulation runs.
            block_number: Some(self.block + 1),
            network_id: self.simulator.0.chain_id.to_string(),
            from: self.request.from.unwrap_or_default(),
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
            value: self.request.value,
            simulation_type: Some(tenderly::dto::SimulationType::Full),
            state_objects: Some(
                self.state_overrides
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            *key,
                            tenderly::dto::StateObject::try_from(value.clone())
                                .map_err(|_| ConversionError::StateOverrides)?,
                        ))
                    })
                    .collect::<Result<_, ConversionError>>()?,
            ),
            access_list: self.request.access_list.as_ref().map(Into::into),
            save: Some(true),
            gas_price: None,
            save_if_fails: Some(true),
            transaction_index: None,
            generate_access_list: None,
        })
    }
}

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Debug)]
pub struct TenderlyReport {
    /// Full request object that can be used directly with the Tenderly API
    pub tenderly_request: tenderly::dto::Request,
    /// Shared Tenderly simulation URL for debugging in the dashboard
    pub tenderly_url: Option<String>,
    /// Any error that might have been reported during order simulation
    pub error: Option<String>,
}

pub enum Solver {
    /// Simulation assumes this is an actual solver so no state overrides will
    /// be applied to allow list it explicitly.
    /// If you need a very specific solver setup for your simulation consider
    /// using this and explicitly adding the necessary
    /// [`AccountOverrideRequest`]s using with
    /// [`SimulationBuilder::with_overrides()`].
    OriginUnaltered(Address),
    /// A fake solver for simulation. Uses the provided address or generates a
    /// random one. The simulation builder will automatically set the required
    /// state overrides to give it enough ETH and allow list it as a solver.
    Fake(Option<Address>),
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

/// How clearing prices are determined for the encoded settlement.
pub enum PriceEncoding {
    /// Derive clearing prices directly from the order's limit price.
    ///
    /// Sets `price[sell_token] = buy_amount` and `price[buy_token] =
    /// sell_amount`, exactly satisfying the order's limit with no surplus.
    LimitPrice,
    /// Explicit clearing prices for the order's sell and buy token. Use this
    /// when the prices differ from the order's limit — e.g. in trade
    /// verification where the order amounts are set to always pass the limit
    /// check and the solver's quoted prices are supplied separately.
    Custom { sell_price: U256, buy_price: U256 },
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
    pub(crate) executed_amount: ExecutionAmount,
    pub(crate) price_encoding: PriceEncoding,
}

impl Order {
    pub fn new(data: OrderData) -> Self {
        Self {
            data,
            owner: Address::ZERO,
            signature: Signature::default_with(SigningScheme::Eip1271),
            executed_amount: ExecutionAmount::Remaining,
            price_encoding: PriceEncoding::LimitPrice,
        }
    }

    pub fn with_signature(mut self, owner: Address, signature: Signature) -> Self {
        self.owner = owner;
        self.signature = signature;
        self
    }

    pub fn fill_at(mut self, execution: ExecutionAmount, price: PriceEncoding) -> Self {
        self.executed_amount = execution;
        self.price_encoding = price;
        self
    }
}

/// Configuration for wrapping the settlement in a flashloan or custom wrapper
/// contract chain.
pub enum WrapperConfig {
    Flashloan(Vec<FlashloanRequest>),
    Custom(Vec<WrapperCall>),
    NoWrapper,
}

pub struct FlashloanRequest {
    pub amount: U256,
    pub borrower: Address,
    pub lender: Address,
    pub token: Address,
}

#[derive(Debug)]
pub enum AccountOverrideRequest {
    /// Gives the address a huge amount of ETH.
    SufficientEthBalance(Address),
    /// Allowlists an address as a solver to let it settle orders.
    AuthenticateAsSolver(Address),
    /// Computes necessary state overrides for the requested balance.
    Balance {
        holder: Address,
        token: Address,
        amount: U256,
    },
    /// Deploys the provided code at the requested address.
    Code { account: Address, code: Bytes },
    /// Allows to build fully custom overrides for the most exotic use cases.
    Custom {
        account: Address,
        state: AccountOverride,
    },
    // TODO: add Allowance
}

/// Error returned when a built eth_call simulation could not be converted
/// into a tenderly simulation request.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("simulation does not have a target")]
    MissingTo,
    #[error("could not convert state overrides")]
    StateOverrides,
}

/// Error returned when data needed to build the final simulation was missing,
/// incompatible, or could not be computed ad-hoc.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("no order was added")]
    NoOrder,
    #[error("no solver was set")]
    NoSolver,
    #[error("failed to query filled amount from settlement contract: {0}")]
    FilledAmountQuery(#[from] anyhow::Error),
    #[error("failed to parse app data: {0}")]
    AppDataParse(#[from] serde_json::Error),
    #[error("both wrappers and flashloans cannot be encoded in the same settlement")]
    FlashloanWrappersIncompatible,
}

/// Error returned when two [`AccountOverride`]s set the same field for the same
/// address and cannot be merged.
#[derive(Debug, thiserror::Error)]
pub(crate) enum MergeConflict {
    #[error("both overrides set the ETH balance")]
    Balance,
    #[error("both overrides set the nonce")]
    Nonce,
    #[error("both overrides set the contract code")]
    Code,
    #[error("both overrides replace the full storage state")]
    State,
    #[error("overrides use incompatible storage strategies (state vs state_diff)")]
    StateAndStateDiff,
    #[error("both overrides write storage slot {0}")]
    StateDiffSlot(B256),
}
