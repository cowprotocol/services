use {
    crate::domain::{eth, order},
    alloy::{
        primitives::{Address, U256},
        providers::Provider,
        rpc::types::state::{AccountOverride, StateOverride},
    },
    balance_overrides::BalanceOverrideRequest,
    contracts::support::{AnyoneAuthenticator, Trader},
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    number::nonzero::NonZeroU256,
    simulator::{
        encoding::WrapperCall,
        swap_simulator::{Query, SwapSimulator, TradeEncoding},
    },
};

pub struct TxGasEstimator {
    simulator: SwapSimulator,
}

impl TxGasEstimator {
    pub fn new(simulator: SwapSimulator) -> Self {
        Self { simulator }
    }

    /// Estimates the gas for settling an order by simulating the full
    /// settlement transaction (including order hooks). Returns `None` if
    /// simulation fails, in which case the caller should fall back to static
    /// gas estimation.
    pub async fn estimate(
        &self,
        order: &order::Order,
        input: eth::Asset,
        output: eth::Asset,
    ) -> Option<eth::Gas> {
        let sell_amount = NonZeroU256::new(input.amount)?;
        let solver = Address::random();
        let owner = order.owner();

        let query = Query {
            sell_token: input.token.0,
            sell_amount,
            buy_token: output.token.0,
            buy_amount: output.amount,
            kind: match order.side {
                order::Side::Sell => OrderKind::Sell,
                order::Side::Buy => OrderKind::Buy,
            },
            receiver: owner,
            sell_token_source: SellTokenSource::Erc20,
            buy_token_destination: BuyTokenDestination::Erc20,
            from: owner,
            tx_origin: None,
            solver,
            tokens: vec![input.token.0, output.token.0],
            clearing_prices: vec![output.amount, input.amount],
            wrappers: order
                .wrappers
                .iter()
                .map(|w| WrapperCall {
                    address: w.address,
                    data: w.data.clone().into(),
                })
                .collect(),
        };

        let mut swap = self
            .simulator
            .fake_swap(&query, TradeEncoding::Simple)
            .await
            .ok()?;

        // Inject order hooks before/after existing interactions.
        let pre = order.pre_interactions.iter().map(encode_interaction);
        swap.settlement.interactions.pre = pre
            .chain(std::mem::take(&mut swap.settlement.interactions.pre))
            .collect();
        swap.settlement
            .interactions
            .post
            .extend(order.post_interactions.iter().map(encode_interaction));

        let state_overrides = self.prepare_state_overrides(solver, owner, output).await?;
        swap.overrides.extend(state_overrides);

        // simulate_settle_call gives us back the encoded tx + overrides;
        // re-use those to call eth_estimateGas.
        let sim = self
            .simulator
            .simulate_settle_call_on_latest(swap)
            .await
            .ok()?;
        let block = *self.simulator.current_block.borrow();
        let gas: u64 = self
            .simulator
            .web3
            .provider
            .estimate_gas(sim.tx)
            .overrides(sim.overrides)
            .block(block.number.into())
            .await
            .ok()?;

        Some(eth::Gas(U256::from(gas)))
    }

    async fn prepare_state_overrides(
        &self,
        solver: Address,
        owner: Address,
        output: eth::Asset,
    ) -> Option<StateOverride> {
        let mut overrides = StateOverride::default();

        let authenticator = self
            .simulator
            .settlement
            .authenticator()
            .call()
            .await
            .ok()?;
        overrides.insert(
            authenticator,
            AccountOverride {
                code: Some(AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );
        overrides.insert(
            solver,
            AccountOverride {
                balance: Some(U256::MAX / U256::from(2)),
                ..Default::default()
            },
        );
        overrides.insert(
            owner,
            AccountOverride {
                code: Some(Trader::Trader::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );
        if let Some((token, balance_override)) = self
            .simulator
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: output.token.0,
                holder: *self.simulator.settlement.address(),
                amount: output.amount,
            })
            .await
        {
            overrides.insert(token, balance_override);
        }

        Some(overrides)
    }
}

fn encode_interaction(i: &eth::Interaction) -> simulator::encoding::EncodedInteraction {
    (i.target, i.value.0, i.calldata.clone().into())
}
