use {
    crate::encoding::{
        EncodedSettlement,
        Interaction,
        Interactions,
        WrapperCall,
        encode_interactions,
        encode_trade,
        encode_wrapper_settlement,
    },
    alloy_primitives::{Address, B256, U256, keccak256},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    alloy_sol_types::SolCall,
    anyhow::Result,
    model::{
        order::{OrderData, OrderKind},
        signature::{Signature, SigningScheme},
    },
    std::sync::Arc,
};

/// A simulator-specific order that bundles the data needed to encode a trade.
///
/// Construct with [`Order::new`] and add optional fields via the builder
/// methods. Defaults to an EIP-1271 signature (pairs with [`FakeUser`] for
/// simulations that need to bypass signature verification).
pub struct Order {
    data: OrderData,
    owner: Address,
    signature: Signature,
    pre_interactions: Vec<Interaction>,
    post_interactions: Vec<Interaction>,
}

impl Order {
    pub fn new(data: OrderData) -> Self {
        Self {
            data,
            owner: Address::ZERO,
            signature: Signature::default_with(SigningScheme::Eip1271),
            pre_interactions: vec![],
            post_interactions: vec![],
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
}

pub struct FlashloanRequest {
    pub amount: U256,
    pub borrower: Address,
    pub lender: Address,
    pub token: Address,
}

pub enum Solver {
    /// A real allow-listed solver address. Used as-is with no state overrides.
    Real(Address),
    /// A fake solver for simulation. Uses the provided address or generates a
    /// random one, then sets its ETH balance to `U256::MAX / 2`.
    Fake(Option<Address>),
}

/// Configuration for wrapping the settlement in a flashloan or custom wrapper
/// contract chain.
pub enum WrapperConfig {
    Flashloan {
        router: Address,
        loans: Vec<FlashloanRequest>,
    },
    Custom(Vec<WrapperCall>),
}

/// How clearing prices are determined for the encoded settlement.
pub enum Prices {
    /// Derive clearing prices directly from the order's limit price.
    ///
    /// Sets `price[sell_token] = buy_amount` and `price[buy_token] =
    /// sell_amount`, exactly satisfying the order's limit with no surplus.
    Limit,
    /// Explicit token list and matching clearing prices.
    Explicit {
        tokens: Vec<Address>,
        clearing_prices: Vec<U256>,
    },
}

/// The output of [`SimulationBuilder::build`]: a transaction request and state
/// overrides ready to be passed to an alloy provider for simulation.
pub struct SettlementCall {
    pub request: TransactionRequest,
    pub state_overrides: StateOverride,
}

/// Assembles a GPv2 settlement call for simulation purposes.
///
/// Call [`SimulationBuilder::build`] when done to produce a [`SettlementCall`].
pub struct SimulationBuilder {
    order: Option<Order>,
    pre_interactions: Vec<Interaction>,
    main_interactions: Vec<Interaction>,
    post_interactions: Vec<Interaction>,
    wrapper: Option<WrapperConfig>,
    prices: Option<Prices>,
    solver: Option<Solver>,
    auction_id: Option<i64>,
    state_overrides: StateOverride,
    simulator: SettlementSimulator,
}

impl SimulationBuilder {
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

    pub fn build(self) -> Result<SettlementCall, BuildError> {
        self.build_with_modifications(|_| {})
    }

    pub fn build_with_modifications(
        self,
        customize: impl FnOnce(&mut EncodedSettlement),
    ) -> Result<SettlementCall, BuildError> {
        let order = self.order.as_ref().ok_or(BuildError::NoOrder)?;

        let (tokens, clearing_prices) = match &self.prices {
            Some(Prices::Explicit {
                tokens,
                clearing_prices,
            }) => (tokens.clone(), clearing_prices.clone()),
            // At limit price: price[sell_token] = buy_amount, price[buy_token] = sell_amount.
            // This makes sell_amount * price[sell] / price[buy] = buy_amount exactly.
            _ => (
                vec![order.data.sell_token, order.data.buy_token],
                vec![order.data.buy_amount, order.data.sell_amount],
            ),
        };

        let sell_token_index = tokens
            .iter()
            .position(|t| *t == order.data.sell_token)
            .ok_or(BuildError::MissingSellToken)?;
        let buy_token_index = tokens
            .iter()
            .position(|t| *t == order.data.buy_token)
            .ok_or(BuildError::MissingBuyToken)?;

        let executed_amount = match order.data.kind {
            OrderKind::Sell => order.data.sell_amount,
            OrderKind::Buy => order.data.buy_amount,
        };

        let trade = encode_trade(
            &order.data,
            &order.signature,
            order.owner,
            sell_token_index,
            buy_token_index,
            executed_amount,
        );

        let order_pre = &order.pre_interactions;
        let order_post = &order.post_interactions;

        let mut settlement = EncodedSettlement {
            tokens,
            clearing_prices,
            trades: vec![trade],
            interactions: Interactions {
                // order's pre-hooks run before any additional pre-interactions
                pre: encode_interactions(order_pre.iter().chain(&self.pre_interactions)),
                main: encode_interactions(&self.main_interactions),
                // additional post-interactions run before the order's post-hooks
                post: encode_interactions(self.post_interactions.iter().chain(order_post)),
            },
        };

        customize(&mut settlement);

        let settle_calldata = {
            let mut bytes = settlement.into_settle_call().to_vec();
            if let Some(id) = self.auction_id {
                bytes.extend_from_slice(&id.to_be_bytes());
            }
            bytes.into()
        };

        let (to, input) = match self.wrapper {
            Some(WrapperConfig::Custom(wrappers)) if !wrappers.is_empty() => {
                encode_wrapper_settlement(&wrappers, settle_calldata)
                    .expect("wrappers is non-empty")
            }
            Some(WrapperConfig::Flashloan { router, loans }) => {
                let calldata =
                    contracts::FlashLoanRouter::FlashLoanRouter::flashLoanAndSettleCall {
                        loans: loans
                            .into_iter()
                            .map(|l| contracts::FlashLoanRouter::LoanRequest::Data {
                                amount: l.amount,
                                borrower: l.borrower,
                                lender: l.lender,
                                token: l.token,
                            })
                            .collect(),
                        settlement: settle_calldata,
                    }
                    .abi_encode()
                    .into();
                (router, calldata)
            }
            _ => (*self.simulator.0.settlement.address(), settle_calldata),
        };

        let mut state_overrides = self.state_overrides;
        let from = match self.solver {
            Some(Solver::Real(addr)) => addr,
            Some(Solver::Fake(opt)) => {
                let addr = opt.unwrap_or_else(Address::random);
                // give solver address enough ETH
                state_overrides.insert(
                    addr,
                    AccountOverride {
                        balance: Some(U256::MAX / U256::from(2)),
                        ..Default::default()
                    },
                );

                // add address to solver allow-list
                let target_slot = {
                    // authenticator stores a `mapping(address=>bool)` in storage
                    // slot 1 so we can compute precisely which storage slot we
                    // have to override
                    let mut buf = [0; 64];
                    buf[12..32].copy_from_slice(addr.as_slice());
                    buf[32..64].copy_from_slice(&U256::ONE.to_be_bytes::<32>());
                    keccak256(buf)
                };
                state_overrides.insert(
                    self.simulator.0.authenticator,
                    AccountOverride {
                        state_diff: Some(
                            // true is encoded as value with the last bit being 1
                            std::iter::once((target_slot, B256::with_last_byte(1))).collect(),
                        ),
                        ..Default::default()
                    },
                );
                addr
            }
            None => return Err(BuildError::NoSolver),
        };

        Ok(SettlementCall {
            request: TransactionRequest {
                from: Some(from),
                to: Some(to.into()),
                input: input.into(),
                ..Default::default()
            },
            state_overrides,
        })
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
}

struct Inner {
    settlement: contracts::GPv2Settlement::Instance,
    authenticator: Address,
}

/// Holds the settlement contract and its authenticator address, and acts as a
/// factory for [`SimulationBuilder`] instances that are pre-configured with
/// these values.
#[derive(Clone)]
pub struct SettlementSimulator(Arc<Inner>);

impl SettlementSimulator {
    pub async fn new(settlement: contracts::GPv2Settlement::Instance) -> Result<Self> {
        let authenticator = Address(settlement.authenticator().call().await?.0);
        Ok(Self(Arc::new(Inner {
            settlement,
            authenticator,
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
        }
    }
}
