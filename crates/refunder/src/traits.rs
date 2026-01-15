//! Trait definitions for database and blockchain access.

#![allow(async_fn_in_trait)]

use {
    alloy::primitives::{Address, B256},
    anyhow::Result,
    contracts::alloy::CoWSwapEthFlow::{self, EthFlowOrder},
    database::{OrderUid, ethflow_orders::EthOrderPlacement},
};

const NO_OWNER: Address = Address::ZERO;
const INVALIDATED_OWNER: Address = Address::repeat_byte(0xff);

/// Status of an EthFlow order refund eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundStatus {
    /// Order has already been refunded or cancelled.
    Refunded,
    /// Order is still active and eligible for refund, with the given owner
    /// address.
    NotYetRefunded(Address),
    /// Order is invalid (never created, already freed, or owner cannot receive
    /// ETH).
    Invalid,
}

impl From<CoWSwapEthFlow::CoWSwapEthFlow::ordersReturn> for RefundStatus {
    fn from(value: CoWSwapEthFlow::CoWSwapEthFlow::ordersReturn) -> Self {
        match value.owner {
            NO_OWNER => Self::Invalid,
            INVALIDATED_OWNER => Self::Refunded,
            owner => Self::NotYetRefunded(owner),
        }
    }
}

/// Database read operations.
#[cfg_attr(test, mockall::automock)]
pub trait DbRead: Send + Sync {
    /// Fetches orders eligible for refund (expired, not invalidated, not
    /// filled, meets price deviation threshold).
    async fn get_refundable_orders(
        &self,
        block_time: i64,
        min_validity_duration: i64,
        min_price_deviation: f64,
    ) -> Result<Vec<EthOrderPlacement>>;

    /// Fetches the EthFlow order data for `uid`.
    async fn get_ethflow_order_data(&self, uid: &OrderUid) -> Result<EthFlowOrder::Data>;
}

/// Blockchain read operations.
#[cfg_attr(test, mockall::automock)]
pub trait ChainRead: Send + Sync {
    /// Returns the current block's timestamp.
    async fn current_block_timestamp(&self) -> Result<u32>;

    /// Returns `true` if `address` can receive ETH (simulates a 1 wei
    /// transfer).
    async fn can_receive_eth(&self, address: Address) -> bool;

    /// Returns the configured EthFlow contract addresses.
    fn ethflow_addresses(&self) -> Vec<Address>;

    /// Queries the on-chain refund status of an order.
    async fn get_order_status(
        &self,
        ethflow_address: Address,
        order_hash: B256,
    ) -> Result<RefundStatus>;
}

/// Blockchain write operations.
#[cfg_attr(test, mockall::automock)]
pub trait ChainWrite: Send + Sync {
    /// Submits a batch refund transaction.
    async fn submit_batch(
        &mut self,
        uids: &[OrderUid],
        encoded_ethflow_orders: Vec<EthFlowOrder::Data>,
        ethflow_contract: Address,
    ) -> Result<()>;
}
