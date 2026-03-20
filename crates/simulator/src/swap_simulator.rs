use {
    crate::encoding::{
        EncodedSettlement,
        EncodedTrade,
        Interaction,
        InteractionEncoding,
        Interactions,
        WrapperCall,
        encode_trade,
        encode_wrapper_settlement,
    },
    alloy_primitives::{Address, U256, address},
    alloy_rpc_types::{TransactionRequest, state::StateOverride},
    alloy_sol_types::SolCall,
    anyhow::{Context, Result, anyhow},
    balance_overrides::BalanceOverriding,
    contracts::alloy::{GPv2Settlement, WETH9, support::Solver},
    eth_domain_types::NonZeroU256,
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    model::{
        DomainSeparator,
        order::{BUY_ETH_ADDRESS, BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, SigningScheme},
    },
    std::sync::Arc,
};

/// Query for the Swap Simulator to prepare a fake settlement with
/// Contains the minimum data required to encode a fake settlement
#[derive(Debug)]
pub struct Query {
    /// The input token, transferred into settlement contract
    pub in_token: Address,
    pub in_amount: NonZeroU256,
    /// The output token, transferred out of settlemetn contract
    pub out_token: Address,
    pub out_amount: U256,
    pub kind: OrderKind,
    pub receiver: Address,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    pub from: Address,
    pub tx_origin: Option<Address>,
    pub solver: Address,
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub wrappers: Vec<WrapperCall>,
}

#[derive(Clone)]
pub struct SwapSimulator {
    pub balance_overrides: Arc<dyn BalanceOverriding>,
    pub settlement: GPv2Settlement::Instance,
    pub native_token: Address,
    pub domain_separator: DomainSeparator,
    pub current_block: CurrentBlockWatcher,
    pub web3: Web3,
    pub gas_limit: u64,
}

pub struct EncodedSwap {
    pub settlement: EncodedSettlement,
    pub overrides: StateOverride,
    pub wrappers: Vec<WrapperCall>,
    pub solver: Address,
    pub receiver: Address,
}

// Look into driver encoding logic for wrappers
pub struct SwapSimulation {
    pub tx: TransactionRequest,
    pub overrides: StateOverride,
    pub result: Result<Solver::Solver::swapReturn, anyhow::Error>,
}

impl SwapSimulator {
    const SPARDOSE: Address = address!("0000000000000000000000000000000000020000");

    pub async fn new(
        balance_overrides: Arc<dyn BalanceOverriding>,
        settlement: Address,
        native_token: Address,
        current_block: CurrentBlockWatcher,
        web3: Web3,
        gas_limit: u64,
    ) -> Result<Self> {
        let settlement = GPv2Settlement::GPv2Settlement::new(settlement, web3.provider.clone());
        let domain_separator = DomainSeparator(settlement.domainSeparator().call().await?.0);

        Ok(Self {
            balance_overrides,
            settlement,
            native_token,
            current_block,
            web3,
            gas_limit,
            domain_separator,
        })
    }

    /// Creates a fake swap based on the provided query
    /// The result can be further post processed depending on the needs
    ///
    /// It can then be simulated with SwapSimulator::simulate_swap
    pub async fn fake_swap(&self, query: &Query) -> Result<EncodedSwap> {
        let overrides = StateOverride::default();

        let pre_interactions = vec![self.trade_setup_interaction(&query).encode()];
        let mut interactions = vec![];

        if query.out_token == BUY_ETH_ADDRESS {
            // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
            // does not have to emit unwraps to pay out `ETH` in a trade.
            // However, for the simulation to be successful this has to happen so we do it
            // ourselves here.
            let buy_amount = match query.kind {
                OrderKind::Sell => query.out_amount,
                OrderKind::Buy => query.in_amount.get(),
            };
            interactions.push((
                self.native_token,
                U256::ZERO,
                WETH9::WETH9::withdrawCall { wad: buy_amount }
                    .abi_encode()
                    .into(),
            ));
            tracing::trace!("adding unwrap interaction for paying out ETH");
        }

        Ok(EncodedSwap {
            settlement: EncodedSettlement {
                tokens: query.tokens.to_vec(),
                clearing_prices: query.clearing_prices.to_vec(),
                trades: vec![encode_fake_trade(&query)?],
                interactions: Interactions {
                    pre: pre_interactions,
                    main: interactions,
                    post: vec![],
                },
            },
            solver: query.tx_origin.unwrap_or(query.solver),
            receiver: query.receiver,
            overrides,
            wrappers: query.wrappers.clone(),
        })
    }

    /// Simulates a solver call to settlement contract with the provided swap
    /// data. The swap call result is contained in the returned
    /// SwapSimulation struct, along with the original TransactionRequest
    /// and State overrides (if needed to be logged, or processed elsewhere)
    pub async fn simulate_swap(&self, swap: EncodedSwap) -> Result<SwapSimulation> {
        let block = *self.current_block.borrow();
        let solver = Solver::Instance::new(swap.solver, self.web3.provider.clone());

        // For wrapped settlements, the Solver contract must call the first wrapper
        // (not the settlement directly). The wrapper then chains to the settlement.
        // For non-wrapped settlements, the Solver calls the settlement contract
        // directly. The transaction always targets the solver contract (never
        // the wrapper directly).
        let (settlement_target, calldata) = if !swap.wrappers.is_empty() {
            encode_wrapper_settlement(&swap.wrappers, swap.settlement.into_settle_call())
        } else {
            (
                *self.settlement.address(),
                swap.settlement.into_settle_call(),
            )
        };

        let overrides = swap.overrides;
        let swap = solver
            .swap(
                settlement_target,
                swap.settlement.tokens.clone(),
                swap.receiver,
                calldata,
            )
            .from(swap.solver)
            .gas(self.gas_limit)
            .gas_price(
                u128::try_from(block.gas_price.saturating_mul(U256::from(2)))
                    .map_err(|err| anyhow!(err))
                    .context("converting gas price to u128")?,
            );

        // Save the transaction request, so the caller can inspect it.
        // For example, to create a tenderly API request and provide the ability to
        // simulate it.
        let tx = swap.clone().into_transaction_request();
        let result = swap
            .call()
            .overrides(overrides.clone())
            .await
            .map_err(|err| anyhow!(err))
            .context("failed to simulate swap");

        Ok(SwapSimulation {
            tx,
            overrides,
            result,
        })
    }

    /// Create interaction that sets up the trade right before transfering
    /// funds. This interaction does nothing if the user-provided
    /// pre-interactions already set everything up (e.g. approvals,
    /// balances). That way we can correctly verify quotes with or without
    /// these user pre-interactions with helpful error messages.
    fn trade_setup_interaction(&self, query: &Query) -> Interaction {
        let sell_amount = match query.kind {
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => query.out_amount,
        };
        let setup_call = Solver::Solver::ensureTradePreconditionsCall {
            trader: query.from,
            settlementContract: *self.settlement.address(),
            sellToken: query.in_token,
            sellAmount: sell_amount,
            nativeToken: self.native_token,
            spardose: Self::SPARDOSE,
        }
        .abi_encode();
        Interaction {
            target: query.solver,
            value: U256::ZERO,
            data: setup_call,
        }
    }
}

/// Encodes a trade with the most disadvantageous in and out amounts possible
/// (while taking possible overflows into account). Should the trader not
/// receive the amount promised by the [`Trade`] the simulation will still work
/// and the actual [`Trade::out_amount`] can be computed afterwards.
fn encode_fake_trade(query: &Query) -> Result<EncodedTrade> {
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Sell => (query.in_amount.get(), U256::ZERO),
        OrderKind::Buy => (
            (query.out_amount).max(U256::from(u128::MAX)),
            query.in_amount.get(),
        ),
    };
    let fake_order = OrderData {
        sell_token: query.in_token,
        sell_amount,
        buy_token: query.out_token,
        buy_amount,
        receiver: Some(query.receiver),
        valid_to: u32::MAX,
        app_data: Default::default(),
        fee_amount: U256::ZERO,
        kind: query.kind,
        partially_fillable: false,
        sell_token_balance: query.sell_token_source,
        buy_token_balance: query.buy_token_destination,
    };

    let fake_signature = Signature::default_with(SigningScheme::Eip1271);
    let encoded_trade = encode_trade(
        &fake_order,
        &fake_signature,
        query.from,
        // the tokens set length is small so the linear search is acceptable
        query
            .tokens
            .iter()
            .position(|token| token == &query.in_token)
            .context("missing sell token index")?,
        query
            .tokens
            .iter()
            .position(|token| token == &query.out_token)
            .context("missing buy token index")?,
        query.in_amount.get(),
    );

    Ok(encoded_trade)
}
