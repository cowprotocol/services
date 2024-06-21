mod implementations;
mod registry;

use {ethcontract::Address, std::sync::Arc};
pub use {
    implementations::safe_based::event_handler::Contract as CowAmmSafeBasedContract,
    registry::Registry,
};

pub trait CowAmm: Send + Sync {
    /// Address of the CoW AMM.
    /// Can be used by the autopilot to build the list of accepted cow amms.
    fn address(&self) -> &Address;

    /// Returns the list of tokens traded by this pool.
    /// Can be used by the autopilot to build the list of native token prices to
    /// query.
    fn traded_tokens(&self) -> &[Address; 2];
}

#[async_trait::async_trait]
pub trait Deployment: Sync + Send {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self) -> Option<Arc<dyn CowAmm>>;
}
