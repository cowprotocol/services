use {
    anyhow::anyhow,
    async_trait::async_trait,
    model::order::Order,
    shared::order_validation::{OrderSimulating, OrderSimulationError},
    simulator::simulation_builder::{
        self,
        Block,
        ExecutionAmount,
        PriceEncoding,
        SettlementSimulator,
        Solver,
    },
};

/// Drives `SettlementSimulator` to run a full-order simulation at order
/// creation time, including pre/post hooks, swap, and any wrapper chain.
pub struct OrderSimulatorAdapter {
    inner: SettlementSimulator,
}

impl OrderSimulatorAdapter {
    pub fn new(inner: SettlementSimulator) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl OrderSimulating for OrderSimulatorAdapter {
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: String,
    ) -> Result<(), OrderSimulationError> {
        let inputs = self
            .inner
            .new_simulation_builder()
            .with_orders([simulation_builder::Order::new(order.data)
                .with_signature(order.metadata.owner, order.signature.clone())
                .fill_at(ExecutionAmount::Full, PriceEncoding::LimitPrice)])
            .parameters_from_app_data(&full_app_data)
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("parse app data")))?
            .from_solver(Solver::Fake(None))
            .provide_sufficient_buy_tokens()
            .at_block(Block::Latest)
            .build()
            .await
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("build")))?;

        // Capture the Tenderly handle and the diagnostic request before
        // consuming `inputs` with `simulate()`. The Tenderly call is only
        // dispatched on revert, since the URL is only useful for diagnostics
        // and most simulations succeed.
        let tenderly = inputs.simulator.tenderly();
        let tenderly_request = inputs.to_tenderly_request().ok();

        match inputs.simulate().await {
            Ok(_) => Ok(()),
            Err(err) => {
                let tenderly_url = match (tenderly, tenderly_request) {
                    (Some(api), Some(req)) => api.simulate_and_share(req).await.ok(),
                    _ => None,
                };
                Err(OrderSimulationError::Reverted {
                    reason: err.to_string(),
                    tenderly_url,
                })
            }
        }
    }
}
