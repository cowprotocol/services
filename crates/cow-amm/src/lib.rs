mod implementations;
mod registry;

use {ethcontract::Address, std::sync::Arc};
pub use {
    implementations::standalone::factory::Contract as CowAmmStandaloneFactory,
    registry::Registry,
};

pub trait CowAmm: Send + Sync {
    /// Address of the CoW AMM.
    fn address(&self) -> &Address;

    /// Returns all tokens traded by this pool in stable order.
    fn traded_tokens(&self) -> &[Address];
}

pub trait Deployment: Sync + Send {
    /// Returns the AMM deployed in the given Event.
    fn deployed_amm(&self) -> Option<Arc<dyn CowAmm>>;
}
