use {
    anyhow::anyhow,
    async_trait::async_trait,
    model::order::Order,
    shared::order_validation::{Eip1271Simulating, Eip1271SimulationError},
    simulator::simulation_builder::{
        self,
        AccountOverrideRequest,
        Block,
        ExecutionAmount,
        Prices,
        SettlementSimulator,
        Solver,
    },
};

/// Drives `SettlementSimulator` to run a full-order simulation for an
/// EIP-1271 order at creation time. Used by the orderbook's signature +
/// simulation matrix.
pub struct OrderSimulatorAdapter {
    inner: SettlementSimulator,
}

impl OrderSimulatorAdapter {
    pub fn new(inner: SettlementSimulator) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl Eip1271Simulating for OrderSimulatorAdapter {
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: String,
    ) -> Result<(), Eip1271SimulationError> {
        let inputs = self
            .inner
            .new_simulation_builder()
            .add_order(
                simulation_builder::Order::new(order.data)
                    .with_signature(order.metadata.owner, order.signature.clone())
                    .with_executed_amount(ExecutionAmount::Full),
            )
            .parameters_from_app_data(&full_app_data)
            .map_err(|err| Eip1271SimulationError::Infra(anyhow!(err).context("parse app data")))?
            .with_prices(Prices::Limit)
            .from_solver(Solver::Fake(None))
            .with_override(AccountOverrideRequest::BuyTokensForBuffers)
            .at_block(Block::Latest)
            .build()
            .await
            .map_err(|err| Eip1271SimulationError::Infra(anyhow!(err).context("build")))?;

        let report = inputs
            .simulate_with_tenderly_report()
            .await
            .map_err(|err| Eip1271SimulationError::Infra(err.context("simulate")))?;

        match report.error {
            Some(reason) => Err(Eip1271SimulationError::Reverted {
                reason,
                tenderly_url: report.tenderly_url,
            }),
            None => Ok(()),
        }
    }
}
