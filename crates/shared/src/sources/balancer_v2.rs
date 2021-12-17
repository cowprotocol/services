//! Contains event handling for maintaining in-memory storage of a `BalancerPoolRegistry` along
//! with tools for retrieving known pools from this registry on demand.
//!
//! While the static information of the pools (such as `pool_id`, `address`, `tokens`) can be
//! kept in memory as part of the registry, their dynamic information (such as current reserves)
//! is block-dependent and must be queried from the EVM upon request.
//!
//! For this we provide `BalancerPoolFetcher` which is responsible for retrieving requested pools
//! from the registry and attaching the most recent reserves to the result.
//!
//! The module is designed to return the most recent pool info on demand.
//! The only public facing components necessary to achieve this are:
//!
//! 1. `BalancerPoolRegistry` which contains an event handler for each distinct Balancer Pool
//! Factory contract and maintains its own in-memory storage of each pool and its static information.
//!
//! 2. `BalancerPoolFetcher` which holds an instance of `BalancerPoolRegistry`,
//! implements `WeightedPoolFetching` and thus exposes a `fetch` method
//! which returns a collection of relevant `WeightedPools` for a given collection of `TokenPair`.
//!
//! 3. `WeightedPool` & `StablePool`:
//!     This is the public facing pool structure returned by the `PoolFetcher` consisting of all
//!     the pool's most recent information (both static and dynamic).
//!     Essentially, this is all the relevant data from `RegisteredWeightedPool` and
//!     `RegisteredStablePool` respectively along with the
//!     current balances of each of the pool's tokens (aka the pool's "reserves").
//!
//! For this reason, only the `event_handler`, `pool_cache`, `pool_fetching` and `swap`
//! are declared as public, others merely contain internal logic regarding how
//! information is collected and stored.
//!
//! Once should think of `PoolStorage` as a type of Database for which one is not concerned
//! with how it maintains itself.

mod graph_api;
pub mod pool_fetching;
mod pool_init;
mod pool_storage;
pub mod pools;
pub mod swap;
