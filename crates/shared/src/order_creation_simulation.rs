use {
    alloy::primitives::{Address, U256},
    anyhow::anyhow,
    async_trait::async_trait,
    contracts::ERC20::ERC20,
    model::{
        order::{Order, OrderKind},
        signature::SigningScheme,
    },
    simulator::{
        report::{Event, SimulationReport},
        simulation_builder::{
            self,
            AccountOverrideRequest,
            Block,
            EthCallInputs,
            ExecutionAmount,
            PriceEncoding,
            SettlementSimulator,
            Solver,
        },
        tenderly,
    },
};

/// Outcome of the order creation simulation.
#[derive(Debug)]
pub enum OrderSimulationError {
    /// The simulation ran and the transaction reverted. `reason` is the
    /// revert string returned by the EVM (or a Tenderly reason string).
    /// `tenderly_request` carries the full payload (calldata, state
    /// overrides, block) needed to replay the simulation manually or against
    /// Tenderly's API, independent of whether `tenderly_url` was produced.
    /// Boxed because the request DTO is large enough that an inline copy
    /// would blow up `Result<(), OrderSimulationError>`'s stack footprint.
    Reverted {
        reason: String,
        summary: Vec<Event>,
        tenderly_url: Option<String>,
        tenderly_request: Option<Box<tenderly::dto::Request>>,
    },
    /// The simulation could not run (RPC failure, Tenderly error, malformed
    /// input, timeout). Treated as fail-open.
    Infra(anyhow::Error),
}

/// Simulates an order's pre-hooks, swap, and post-hooks against the chain.
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait]
pub trait OrderSimulating: Send + Sync {
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: &str,
        full_balance_check: bool,
    ) -> Result<(), OrderSimulationError>;
}

/// Drives [`SettlementSimulator`] to run a full-order simulation at order
/// creation time, including pre/post hooks, swap, and any wrapper chain.
pub struct OrderCreationSimulator {
    inner: SettlementSimulator,
}

impl OrderCreationSimulator {
    pub fn new(inner: SettlementSimulator) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl OrderSimulating for OrderCreationSimulator {
    #[tracing::instrument(skip_all, fields(order_uid = %order.metadata.uid))]
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: &str,
        full_balance_check: bool,
    ) -> Result<(), OrderSimulationError> {
        let inputs = self
            .prepare_simulation(order, full_app_data, full_balance_check)
            .await?;
        analyze_simulation(order, inputs).await
    }
}

impl OrderCreationSimulator {
    async fn prepare_simulation(
        &self,
        order: &Order,
        full_app_data: &str,
        full_balance_check: bool,
    ) -> Result<EthCallInputs, OrderSimulationError> {
        let amount_to_fill = match !order.data.partially_fillable || full_balance_check {
            true => ExecutionAmount::Full,
            false => {
                let sell_token = ERC20::new(order.data.sell_token, self.inner.provider());
                let sell_token_balance = sell_token
                    .balanceOf(order.metadata.owner)
                    .call()
                    .await
                    .unwrap_or_default();
                let clamped_sell_amount =
                    sell_token_balance.clamp(U256::ONE, order.data.sell_amount);
                let final_amount = match order.data.kind {
                    OrderKind::Sell => clamped_sell_amount,
                    // convert maxium sell amount to buy tokens
                    OrderKind::Buy => clamped_sell_amount
                        .checked_mul(order.data.buy_amount)
                        .and_then(|val| val.checked_div(order.data.sell_amount))
                        .unwrap_or(U256::ONE),
                };
                ExecutionAmount::Explicit(final_amount)
            }
        };
        let sim_order = simulation_builder::Order::new(order.data)
            .with_signature(order.metadata.owner, order.signature.clone())
            .fill_at(amount_to_fill, PriceEncoding::LimitPrice);

        self.inner
            .new_simulation_builder()
            .with_orders([sim_order])
            .parameters_from_app_data(full_app_data)
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("parse app data")))?
            .from_solver(Solver::Fake(None))
            .provide_sufficient_buy_tokens()
            .presign_orders()
            .at_block(Block::Latest)
            .build()
            .await
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("build")))
    }
}

/// Runs the simulation, handles errors caused by fundamental limitations of
/// the simulation approach and returns a comprehensive report in case of an
/// error.
async fn analyze_simulation(
    order: &Order,
    inputs: EthCallInputs,
) -> Result<(), OrderSimulationError> {
    let report = inputs
        .simulation_report()
        .await
        .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("simulate")))?;

    let revert_reason = match extract_critical_revert(
        order,
        inputs.simulator.settlement_address(),
        &inputs.failed_state_overrides,
        &report,
    ) {
        Some(revert) => revert,
        None => return Ok(()),
    };

    let tenderly = inputs.simulator.tenderly();
    let tenderly_request = inputs.to_tenderly_request().ok();
    let tenderly_url = match (tenderly, tenderly_request.as_ref()) {
        (Some(api), Some(req)) => api.simulate_and_share(req.clone()).await.ok(),
        _ => None,
    };
    Err(OrderSimulationError::Reverted {
        reason: revert_reason,
        summary: report.events,
        tenderly_url,
        tenderly_request: tenderly_request.map(Box::new),
    })
}

/// Analyzes the simulation and only surfaces a revert if it was critical
/// (as opposed to being caused by limitations of the simulation
/// approach).
fn extract_critical_revert(
    order: &Order,
    settlement: Address,
    failed_state_overrides: &[AccountOverrideRequest],
    report: &SimulationReport,
) -> Option<String> {
    let Some(revert) = &report.revert else {
        return None;
    };

    if disallow_broken_hook(report) {
        return Some(revert.clone());
    }

    if allow_failed_buy_token_transfer(order, settlement, failed_state_overrides, report) {
        tracing::debug!(
            summary = ?report.events,
            ?failed_state_overrides,
            "allow reverting order: buy token balance override failed"
        );
        return None;
    }

    if allow_unfunded_presign_order(order, settlement, report) {
        tracing::debug!(
            summary = ?report.events,
            "allow reverting order: unfunded presign order"
        );
        return None;
    }

    Some(revert.clone())
}

/// Because the trampoline contract catches reverts of hooks they will not
/// cause the settlement to revert outright. However, usually hooks are
/// required for an order to work so the revert that's ultimately caused by
/// the broken hook will only in a settlement revert later (e.g. when
/// flashloan can't be paid back correctly). Let's assume all hooks are
/// required and reject orders with broken ones.
fn disallow_broken_hook(report: &SimulationReport) -> bool {
    report
        .events
        .iter()
        .any(|e| matches!(e, Event::Hook { caught_error, .. } if caught_error.is_some()))
}

/// The simulation requires the settlement contract to have enough buy
/// tokens to pay out the order directly. But it's also possible that
/// we fail to compute the necessary state overrides for that.
/// To not prevent the creation of reasonable orders we allow placing
/// orders where ONLY the buy token transfer reverted AND we failed to
/// compute the required overrides.
fn allow_failed_buy_token_transfer(
    order: &Order,
    settlement: Address,
    failed_state_overrides: &[AccountOverrideRequest],
    report: &SimulationReport,
) -> bool {
    let buy_token_transfer_failed = report.events.iter().any(|e| {
        matches!(e,
            Event::Transfer { from, token, to, revert, .. }
                if revert.is_some()
                && from == &settlement
                && token == &order.data.buy_token
                && to == &order.data.receiver.unwrap_or(order.metadata.owner))
    });
    let buy_token_override_failed = failed_state_overrides.iter().any(|o| {
        matches!(o,
            AccountOverrideRequest::Balance { token, holder, .. }
                if token == &order.data.buy_token
                && holder == &settlement
        )
    });

    buy_token_transfer_failed && buy_token_override_failed
}

/// PreSign orders are currently the only ones that are allowed to be placed
/// without any balance at all. If we detect a revert of the sell token
/// transfer we allow it when it's a pre-sign order.
fn allow_unfunded_presign_order(
    order: &Order,
    settlement: Address,
    report: &SimulationReport,
) -> bool {
    let sell_token_transfer_failed = report.events.iter().any(|e| {
        matches!(e,
            Event::Transfer { from, token, to, revert, .. }
                if revert.is_some()
                && to == &settlement
                && token == &order.data.sell_token
                && from == &order.metadata.owner)
    });

    sell_token_transfer_failed && order.signature.scheme() == SigningScheme::PreSign
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        model::order::OrderBuilder,
        simulator::{
            report::{Event, SimulationReport},
            simulation_builder::AccountOverrideRequest,
        },
    };

    const SETTLEMENT: Address = Address::repeat_byte(0x11);
    const SELL_TOKEN: Address = Address::repeat_byte(0xaa);
    const BUY_TOKEN: Address = Address::repeat_byte(0xbb);
    const OWNER: Address = Address::repeat_byte(0xcc);

    fn report_with_revert(events: Vec<Event>) -> SimulationReport {
        SimulationReport {
            events,
            returned_bytes: None,
            revert: Some("revert".to_owned()),
        }
    }

    fn ok_report() -> SimulationReport {
        SimulationReport {
            events: vec![],
            returned_bytes: None,
            revert: None,
        }
    }

    fn buy_token_transfer_failed() -> Event {
        Event::Transfer {
            token: BUY_TOKEN,
            from: SETTLEMENT,
            to: OWNER,
            amount: U256::from(100),
            revert: Some("transfer failed".to_owned()),
        }
    }

    fn sell_token_transfer_failed() -> Event {
        Event::Transfer {
            token: SELL_TOKEN,
            from: OWNER,
            to: SETTLEMENT,
            amount: U256::from(100),
            revert: Some("transfer failed".to_owned()),
        }
    }

    fn buy_token_override_failed() -> AccountOverrideRequest {
        AccountOverrideRequest::Balance {
            token: BUY_TOKEN,
            holder: SETTLEMENT,
            amount: U256::from(100),
        }
    }

    fn check(
        order: &Order,
        failed_overrides: &[AccountOverrideRequest],
        report: &SimulationReport,
    ) -> Option<String> {
        extract_critical_revert(order, SETTLEMENT, failed_overrides, report)
    }

    #[test]
    fn no_revert_in_report_is_ok() {
        let order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .build();

        assert!(check(&order, &[], &ok_report()).is_none());
    }

    #[test]
    fn detects_unjust_buy_token_transfer_revert() {
        let mut order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .build();
        order.metadata.owner = OWNER;
        let report = report_with_revert(vec![buy_token_transfer_failed()]);

        assert!(check(&order, &[buy_token_override_failed()], &report).is_none());
    }

    #[test]
    fn surfaces_hook_revert_despite_buy_transfer_exception() {
        let mut order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .build();
        order.metadata.owner = OWNER;
        let broken_hook = Event::Hook {
            target: Address::repeat_byte(0x00),
            caught_error: Some("hook failed".to_owned()),
        };
        let report = report_with_revert(vec![broken_hook, buy_token_transfer_failed()]);

        assert!(check(&order, &[buy_token_override_failed()], &report).is_some());
    }

    #[test]
    fn allows_presign_sell_transfer_revert() {
        let order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .with_presign(OWNER)
            .build();
        let report = report_with_revert(vec![sell_token_transfer_failed()]);

        assert!(check(&order, &[], &report).is_none());
    }

    #[test]
    fn rejects_sell_transfer_revert_for_all_other_orders() {
        // EIP-712 order (default) — same failure must surface as a critical revert.
        let order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .build();
        let report = report_with_revert(vec![sell_token_transfer_failed()]);

        assert!(check(&order, &[], &report).is_some());
    }

    #[test]
    fn buy_token_transfer_revert_is_critical_without_state_override_error() {
        // The transfer reverted but we *did* manage to set the balance override,
        // so this is a genuine problem and should not be silenced.
        let mut order = OrderBuilder::default()
            .with_sell_token(SELL_TOKEN)
            .with_buy_token(BUY_TOKEN)
            .build();
        order.metadata.owner = OWNER;
        let report = report_with_revert(vec![buy_token_transfer_failed()]);

        assert!(check(&order, &[], &report).is_some());
    }
}
