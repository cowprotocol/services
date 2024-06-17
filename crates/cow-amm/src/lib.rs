mod cow_amm;
pub mod cow_amm_constant_product_factory;
mod event_updater;
mod indexer;

use ethcontract::Address;
pub use {event_updater::EventUpdater, indexer::Indexer};

pub trait CowAmm {
    /// Address of the CoW AMM.
    /// Can be used by the autopilot to build the list of accepted cow amms.
    fn address(&self) -> &Address;

    /// Returns the list of tokens traded by this pool.
    /// Can be used by the autopilot to build the list of native token prices to
    /// query.
    fn traded_tokens(&self) -> &[Address];

    // /// Computes a CoW protocol order and its required commitment interaction of
    // /// the CoW AMM given a list of reference `prices`. The `prices` need to
    // /// be passed in the same order `traded_tokens` returns the token
    // /// addresses.
    // /// Can be used by the driver to create very short lived template orders for
    // /// solvers to fill in order to rebalance the pools without native CoW AMM
    // support. @TODO: Implement in the upcoming PR
    // async fn tradable_order(
    //     &self,
    //     prices: impl IntoIterator<Item = U256>,
    // ) -> (OrderData, Interaction) {
    //     // For each implementation of CoW AMMs (standalone, balancer, etc.) there
    // is a     // separate helper function that can be used to compute a
    // tradable     // order. This function takes the `prices` and some data
    //     // (`TradingParams`) that is unique to each individual AMM instance.
    //     // These `TradingParams` need to be stored inside the CoW AMM struct.
    //     // The bytes for the `TradingParams` should be recoverable from `enabled`
    // events     todo!()
    // }
}
