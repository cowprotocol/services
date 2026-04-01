use {
    crate::dto::OrderSimulationResult,
    alloy::{
        primitives::{Address, U256},
        rpc::types::state::AccountOverride,
    },
    anyhow::{Context, Result},
    balance_overrides::BalanceOverrideRequest,
    contracts::alloy::support::{AnyoneAuthenticator, Trader},
    eth_domain_types::BlockNo,
    model::order::{Order, OrderKind},
    number::conversions::big_uint_to_u256,
    shared::remaining_amounts,
    simulator::{
        encoding::InteractionEncoding,
        swap_simulator::{EncodedSwap, Query, SwapSimulator, TradeEncoding},
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

    /// Encodes an order for simulation.
    ///
    /// `executed_amount` overrides how much of the order has already been
    /// filled (in the order's fill token: sell token for sell orders, buy
    /// token for buy orders). When `None`, the executed amount is taken from
    /// the order's metadata, which reflects the actual on-chain fill state.
    pub async fn encode_order(
        &self,
        order: &Order,
        executed_amount: Option<U256>,
    ) -> Result<EncodedSwap> {
        let Some(app_data) = &order.metadata.full_app_data else {
            anyhow::bail!("App data is not known for order {}", order.metadata.uid)
        };
        let app_data = serde_json::from_str::<app_data::Root>(app_data)?;

        let executed_amount = executed_amount.unwrap_or_else(|| match order.data.kind {
            OrderKind::Buy => big_uint_to_u256(&order.metadata.executed_buy_amount)
                .unwrap_or(order.data.buy_amount),
            OrderKind::Sell => order.metadata.executed_sell_amount_before_fees,
        });
        let remaining_order = remaining_amounts::Order {
            kind: order.data.kind,
            buy_amount: order.data.buy_amount,
            sell_amount: order.data.sell_amount,
            fee_amount: order.data.fee_amount,
            executed_amount,
            partially_fillable: order.data.partially_fillable,
        };
        let remaining =
            remaining_amounts::Remaining::from_order(&remaining_order).with_context(|| {
                format!(
                    "could not compute remaining amounts for order {}",
                    order.metadata.uid
                )
            })?;
        let remaining_sell = remaining
            .remaining(order.data.sell_amount)
            .context("overflow computing remaining sell amount")?;
        let remaining_buy = remaining
            .remaining(order.data.buy_amount)
            .context("overflow computing remaining buy amount")?;

        let tokens = vec![order.data.sell_token, order.data.buy_token];
        // Clearing prices represent the limit price of the order; both order kinds
        // produce the same ratio: [buy_amount, sell_amount] for [sell_token,
        // buy_token].
        let clearing_prices = vec![order.data.buy_amount, order.data.sell_amount];

        let solver = Address::random();
        let query = Query {
            sell_amount: remaining_sell.try_into()?,
            sell_token: order.data.sell_token,
            buy_amount: remaining_buy,
            buy_token: order.data.buy_token,
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

        let swap = self
            .simulator
            .fake_swap(&query, TradeEncoding::Simple)
            .await?;
        let swap = add_interactions(swap, order);
        let swap = self.add_state_overrides(&query, swap).await?;

        Ok(swap)
    }

    /// Simulates a swap of the provided EncodedSwap.
    ///
    /// The result contains the transaction simulation error (if any)
    /// and a full API request object that can be used to resimulate the swap
    /// using Tenderly.
    pub async fn simulate_swap(
        &self,
        swap: EncodedSwap,
        block_number: Option<u64>,
    ) -> Result<OrderSimulationResult> {
        let block_number =
            block_number.unwrap_or_else(|| self.simulator.current_block.borrow().number);
        let result = self
            .simulator
            .simulate_settle_call(swap, Some(block_number))
            .await?;

        let tenderly_request = simulator::tenderly::dto::Request {
            transaction_index: None,
            save: Some(true),
            save_if_fails: Some(true),
            ..simulator::tenderly::prepare_request(
                self.chain_id.clone(),
                &result.tx,
                result.overrides,
                Some(BlockNo(block_number)),
            )?
        };

        Ok(OrderSimulationResult {
            tenderly_request,
            error: result.result.err().map(|err| err.to_string()),
        })
    }

    pub async fn add_state_overrides(
        &self,
        query: &Query,
        mut swap: EncodedSwap,
    ) -> Result<EncodedSwap> {
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
        swap.overrides.insert(
            query.solver,
            AccountOverride {
                // Allow solver simulations to proceed even if the real account holds no ETH.
                // The number is obscenely large, but not max to avoid potential overflows.
                // We had this set to eth(1), but some simulations require more than that on non-ETH
                // networks e.g. polygon so it led to reverts.
                balance: Some(U256::MAX / U256::from(2)),
                ..Default::default()
            },
        );

        // Override trader address with Trader bytecode so EIP-1271 signature
        // verification works for EOA traders (settlement calls isValidSignature
        // on the trader address, which would revert for plain EOAs).
        swap.overrides.insert(
            query.from,
            AccountOverride {
                code: Some(Trader::Trader::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );

        // Fund the settlement contract with enough buy tokens to be paid out
        match self
            .simulator
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: query.buy_token,
                holder: *self.simulator.settlement.address(),
                amount: query.buy_amount,
            })
            .await
        {
            Some((token, balance_override)) => {
                swap.overrides.insert(token, balance_override);
            }
            None => {
                tracing::warn!("Could not set state balance override for the settlement contract");
            }
        };

        Ok(swap)
    }
}

fn add_interactions(mut swap: EncodedSwap, order: &Order) -> EncodedSwap {
    // Add order pre interactions before encoded swap's pre interactions
    let pre_interactions = order
        .interactions
        .pre
        .iter()
        .map(InteractionEncoding::encode)
        .collect();
    // Prepend order pre_interactions so they run first
    let settlement_pre_interactions =
        std::mem::replace(&mut swap.settlement.interactions.pre, pre_interactions);
    swap.settlement
        .interactions
        .pre
        .extend(settlement_pre_interactions);

    // Add order post interactions after encoded swap's post interactions
    let post_interactions = order
        .interactions
        .post
        .iter()
        .map(InteractionEncoding::encode);
    swap.settlement.interactions.post.extend(post_interactions);

    swap
}
