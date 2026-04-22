use {
    crate::encoding::{
        EncodedSettlement,
        EncodedTrade,
        Interactions,
        WrapperCall,
        encode_trade,
        encode_wrapper_settlement,
    },
    alloy_primitives::{Address, Bytes, U256},
    alloy_provider::Provider,
    alloy_rpc_types::{TransactionRequest, state::StateOverride},
    alloy_sol_types::SolCall,
    anyhow::{Context, Result, anyhow},
    balance_overrides::BalanceOverriding,
    contracts::{
        GPv2Settlement::{self},
        WETH9,
        support::Solver::{self, Solver::swapReturn},
    },
    eth_domain_types::NonZeroU256,
    ethrpc::{
        Web3,
        block_stream::{BlockInfo, CurrentBlockWatcher},
    },
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
    pub sell_token: Address,
    pub sell_amount: NonZeroU256,
    pub buy_token: Address,
    pub buy_amount: U256,
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

/// Controls how the trade is encoded for the provided Query
#[derive(Clone, Debug)]
pub enum TradeEncoding {
    /// Encodes the trade amounts exactly as in the Query
    Simple,
    /// Encodes a trade with the most disadvantageous in and out amounts
    /// possible (while taking possible overflows into account). Should the
    /// trader not receive the amount promised by the [`Query`] the
    /// simulation will still work and the actual out amount can be computed
    /// afterwards.
    Disadvantageous,
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

/// The output of a swap simulation.
///
/// Contains the transaction request that was used to perform the simulation
/// (useful for introspection), The used state overrides and simulation result
/// The result is of generic type O, and depends on the type of simulation:
/// - solver swap simulation returns Solver::swapResult
/// - generic simulation returns Bytes
pub struct SwapSimulation<O> {
    pub tx: TransactionRequest,
    pub overrides: StateOverride,
    pub result: Result<O, anyhow::Error>,
}

impl SwapSimulator {
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
    ///
    /// The trade_encoding controls if the trade should be encoded as-is,
    /// based on the Query or if it should be encoded as the most
    /// disadvantegous trade possible.
    ///
    /// The TradeEncoding::Disadvantegous is useful for price verification since
    /// the resulting out amounts can be calculated later while allowing the
    /// simulation to pass.
    pub async fn fake_swap(
        &self,
        query: &Query,
        trade_encoding: TradeEncoding,
    ) -> Result<EncodedSwap> {
        let overrides = StateOverride::default();

        let pre_interactions = Vec::new();
        let mut interactions = Vec::new();

        if query.buy_token == BUY_ETH_ADDRESS {
            // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
            // does not have to emit unwraps to pay out `ETH` in a trade.
            // However, for the simulation to be successful this has to happen so we do it
            // ourselves here.
            interactions.push((
                self.native_token,
                U256::ZERO,
                WETH9::WETH9::withdrawCall {
                    wad: query.buy_amount,
                }
                .abi_encode()
                .into(),
            ));
            tracing::trace!("adding unwrap interaction for paying out ETH");
        }

        Ok(EncodedSwap {
            settlement: EncodedSettlement {
                tokens: query.tokens.to_vec(),
                clearing_prices: query.clearing_prices.to_vec(),
                trades: vec![encode_fake_trade(query, trade_encoding)?],
                interactions: Interactions {
                    pre: pre_interactions,
                    main: interactions,
                    post: Vec::new(),
                },
            },
            solver: query.tx_origin.unwrap_or(query.solver),
            receiver: query.receiver,
            overrides,
            wrappers: query.wrappers.clone(),
        })
    }

    /// For wrapped settlements, the Solver contract must call the first wrapper
    /// (not the settlement directly). The wrapper then chains to the
    /// settlement. For non-wrapped settlements, the Solver calls the
    /// settlement contract directly.
    fn get_target_and_calldata(&self, swap: &EncodedSwap) -> (Address, Bytes) {
        if !swap.wrappers.is_empty() {
            encode_wrapper_settlement(&swap.wrappers, swap.settlement.into_settle_call())
                .expect("wrappers is not empty")
        } else {
            (
                *self.settlement.address(),
                swap.settlement.into_settle_call(),
            )
        }
    }

    /// Simulates a solver call to settlement contract with the provided swap
    /// data. The swap call result is contained in the returned
    /// SwapSimulation struct, along with the original TransactionRequest
    /// and State overrides (if needed to be logged, or processed elsewhere).
    ///
    /// The caller supplies the `block` so the gas-price computation, the
    /// pinned `.call()` block, and any downstream logging all reference the
    /// same snapshot of chain state.
    pub async fn simulate_swap_with_solver(
        &self,
        swap: EncodedSwap,
        block: BlockInfo,
    ) -> Result<SwapSimulation<swapReturn>> {
        let (settlement_target, calldata) = self.get_target_and_calldata(&swap);
        let solver = Solver::Instance::new(swap.solver, self.web3.provider.clone());
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
            .block(block.number.into())
            .await
            .map_err(|err| anyhow!(err))
            .context("failed to simulate swap");

        Ok(SwapSimulation {
            tx,
            overrides,
            result,
        })
    }

    /// Simulate settle call on the latest block
    pub async fn simulate_settle_call_on_latest(
        &self,
        swap: EncodedSwap,
    ) -> Result<SwapSimulation<Bytes>> {
        let block_number = self.current_block.borrow().number;
        self.simulate_settle_call(swap, block_number).await
    }

    pub async fn simulate_settle_call(
        &self,
        swap: EncodedSwap,
        block_number: u64,
    ) -> Result<SwapSimulation<Bytes>> {
        let (settlement_target, calldata) = self.get_target_and_calldata(&swap);

        let overrides = swap.overrides;
        let tx = TransactionRequest {
            from: Some(swap.solver),
            to: Some(settlement_target.into()),
            input: calldata.into(),
            gas: Some(self.gas_limit),
            ..Default::default()
        };

        let result = self
            .web3
            .provider
            .call(tx.clone())
            .overrides(overrides.clone())
            .block(block_number.into())
            .await
            .map_err(|err| anyhow!(err));

        Ok(SwapSimulation {
            tx,
            overrides,
            result,
        })
    }
}

fn encode_fake_trade(query: &Query, trade_encoding: TradeEncoding) -> Result<EncodedTrade> {
    let (sell_amount, buy_amount) = match trade_encoding {
        TradeEncoding::Simple => (query.sell_amount.into(), query.buy_amount),
        TradeEncoding::Disadvantageous => match query.kind {
            OrderKind::Sell => (query.sell_amount.get(), U256::ZERO),
            OrderKind::Buy => (
                query.sell_amount.get().max(U256::from(u128::MAX)),
                query.buy_amount,
            ),
        },
    };

    let fake_order = OrderData {
        sell_token: query.sell_token,
        sell_amount,
        buy_token: query.buy_token,
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
            .position(|token| token == &query.sell_token)
            .context("missing sell token index")?,
        query
            .tokens
            .iter()
            .position(|token| token == &query.buy_token)
            .context("missing buy token index")?,
        match query.kind {
            OrderKind::Sell => query.sell_amount.get(),
            OrderKind::Buy => query.buy_amount,
        },
    );

    Ok(encoded_trade)
}
