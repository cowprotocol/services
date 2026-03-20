use {
    alloy::{
        primitives::{Address, U256},
        rpc::types::state::AccountOverride,
    },
    anyhow::{Context, Result},
    balance_overrides::BalanceOverrideRequest,
    contracts::alloy::support::{AnyoneAuthenticator, Solver, Trader},
    model::{
        order::{Order, OrderKind},
        order_simulator::OrderSimulation,
    },
    simulator::{
        encoding::{EncodedInteraction, InteractionEncoding},
        swap_simulator::{EncodedSwap, Query, SwapSimulator},
    },
};

pub struct OrderSimulator {
    simulator: SwapSimulator,
    chain_id: String,
}

impl OrderSimulator {
    pub fn new(simulator: SwapSimulator, chain_id: String) -> Self {
        Self {
            simulator,
            chain_id,
        }
    }

    pub async fn encode_order(&self, order: &Order) -> Result<EncodedSwap> {
        let Some(app_data) = &order.metadata.full_app_data else {
            anyhow::bail!("App data is not known for order {}", order.metadata.uid)
        };
        let app_data = serde_json::from_str::<app_data::Root>(app_data)?;
        let (in_amount, out_amount) = match order.data.kind {
            OrderKind::Sell => (order.data.sell_amount, order.data.buy_amount),
            OrderKind::Buy => (order.data.buy_amount, order.data.sell_amount),
        };

        let tokens = vec![order.data.sell_token, order.data.buy_token];
        let clearing_prices = match order.data.kind {
            OrderKind::Sell => {
                vec![out_amount, in_amount]
            }
            OrderKind::Buy => {
                vec![in_amount, out_amount]
            }
        };

        let solver = Address::random();
        let query = Query {
            in_amount: in_amount.try_into()?,
            in_token: order.data.sell_token,
            out_amount,
            out_token: order.data.buy_token,
            kind: order.data.kind,
            receiver: order.data.receiver.unwrap_or(order.metadata.owner),
            sell_token_source: order.data.sell_token_balance,
            buy_token_destination: order.data.buy_token_balance,
            from: order.metadata.owner,
            tx_origin: None,
            clearing_prices,
            solver,
            tokens,
            wrappers: app_data
                .wrappers()
                .iter()
                .map(|wrapper| simulator::encoding::WrapperCall {
                    address: wrapper.address,
                    data: wrapper.data.clone().into(),
                })
                .collect(),
        };
        tracing::error!(?query, "Order simulator");

        let mut swap = self.simulator.fake_swap(&query).await?;
        add_interactions(&mut swap, order);
        self.add_state_overrides(&query, &mut swap).await?;

        Ok(swap)
    }

    pub async fn simulate_swap(&self, swap: EncodedSwap) -> Result<OrderSimulation> {
        let result = self.simulator.simulate_swap(swap).await?;

        let request = simulator::tenderly::dto::Request {
            transaction_index: None,
            save: Some(true),
            save_if_fails: Some(true),
            ..simulator::tenderly::prepare_request(
                self.chain_id.clone(),
                &result.tx,
                result.overrides,
                None,
            )?
        };

        Ok(OrderSimulation {
            tenderly_request: request.into(),
            error: result.result.err().map(|err| err.to_string()),
        })
    }

    pub async fn add_state_overrides(&self, query: &Query, swap: &mut EncodedSwap) -> Result<()> {
        // Override authenticator with AnyoneAuthenticator so our fake solver is
        // accepted.
        let authenticator = self
            .simulator
            .settlement
            .authenticator()
            .call()
            .await
            .context("could not fetch authenticator")?;
        swap.overrides.insert(
            authenticator,
            AccountOverride {
                code: Some(AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );

        // Set up fake solver.
        let solver_override = AccountOverride {
            code: Some(Solver::Solver::DEPLOYED_BYTECODE.clone()),
            // Allow solver simulations to proceed even if the real account holds no ETH.
            // The number is obscenely large, but not max to avoid potential overflows.
            // We had this set to eth(1), but some simulations require more than that on non-ETH
            // netowrks e.g. polygon so it led to reverts.
            balance: Some(U256::MAX / U256::from(2)),
            ..Default::default()
        };
        swap.overrides.insert(query.solver, solver_override);

        // Fund the settlement contract with enough buy tokens to pay out
        self.simulator
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: query.out_token,
                holder: *self.simulator.settlement.address(),
                amount: query.out_amount,
            })
            .await
            .map(|(token, balance_override)| swap.overrides.insert(token, balance_override));

        Ok(())
    }
}

fn add_interactions(swap: &mut EncodedSwap, order: &Order) {
    let pre_interactions: Vec<EncodedInteraction> = order
        .interactions
        .pre
        .iter()
        .map(InteractionEncoding::encode)
        .collect();
    let post_interactions: Vec<EncodedInteraction> = order
        .interactions
        .post
        .iter()
        .map(InteractionEncoding::encode)
        .collect();
    swap.settlement.interactions.pre = pre_interactions
        .into_iter()
        .chain(std::mem::take(&mut swap.settlement.interactions.pre))
        .collect();
    swap.settlement.interactions.post.extend(post_interactions);
}
