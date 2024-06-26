mod implementations;
mod registry;

use {
    anyhow::Result,
    ethcontract::{Address, U256},
    model::{interaction::InteractionData, order::OrderData, signature::Signature},
    std::sync::Arc,
};
pub use {
    implementations::standalone::factory::Contract as CowAmmStandaloneFactory,
    registry::Registry,
};

#[async_trait::async_trait]
pub trait CowAmm: Send + Sync {
    /// Address of the CoW AMM.
    fn address(&self) -> &Address;

    /// Returns all tokens traded by this pool in stable order.
    fn traded_tokens(&self) -> &[Address];

    /// Returns an order to rebalance the AMM based on the provided reference
    /// prices. `prices` need to be computed using a common denominator and
    /// need to be supplied in the same order as `traded_tokens` returns
    /// token addresses.
    async fn template_order(
        &self,
        prices: &[U256],
    ) -> Result<(OrderData, Signature, InteractionData)>;
}

pub trait Deployment: Sync + Send {
    /// Returns the AMM deployed in the given Event.
    fn deployed_amm(&self) -> Option<Arc<dyn CowAmm>>;
}
