//! A top-level containing implementations of clients to other CoW Protocol
//! components.

pub mod fee;
pub mod orderbook;

pub use self::orderbook::Orderbook;
