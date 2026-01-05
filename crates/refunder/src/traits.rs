//! Trait definitions for external system boundaries.
//!
//! These traits abstract database and blockchain interactions to enable
//! unit testing with mocks.

use {
    alloy::primitives::{Address, B256, address},
    anyhow::Result,
    contracts::alloy::CoWSwapEthFlow::{self, EthFlowOrder},
    database::{OrderUid, ethflow_orders::EthOrderPlacement},
};

const NO_OWNER: Address = Address::ZERO;
const INVALIDATED_OWNER: Address = address!("0xffffffffffffffffffffffffffffffffffffffff");

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
            owner if owner == NO_OWNER => Self::Invalid,
            owner if owner == INVALIDATED_OWNER => Self::Refunded,
            owner => Self::NotYetRefunded(owner),
        }
    }
}

/// Abstracts database read operations for the refund service.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait DbRead: Send + Sync {
    /// Retrieves EthFlow orders eligible for refunding.
    ///
    /// Returns orders that:
    /// - Have expired (`valid_to` < `block_time`)
    /// - Have not been refunded
    /// - Have not been invalidated on-chain
    /// - Have not been filled (traded)
    /// - Are not partially fillable
    /// - Meet minimum validity duration
    /// - Meet minimum price deviation from quote
    async fn get_refundable_orders(
        &self,
        block_time: i64,
        min_validity_duration: i64,
        min_price_deviation: f64,
    ) -> Result<Vec<EthOrderPlacement>>;

    /// Retrieves complete order data needed to construct a refund transaction.
    async fn get_ethflow_order_data(&self, uid: &OrderUid) -> Result<EthFlowOrder::Data>;
}

/// Abstracts blockchain read operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ChainRead: Send + Sync {
    /// Returns the timestamp of the current (latest) block in seconds.
    async fn current_block_timestamp(&self) -> Result<u32>;

    /// Checks if an address can receive ETH transfers.
    ///
    /// Returns `true` if the address can receive ETH, `false` otherwise.
    /// Used to filter out contracts that would reject ETH (e.g., EOF
    /// contracts).
    async fn can_receive_eth(&self, address: Address) -> bool;

    /// Returns the addresses of all configured EthFlow contracts.
    fn ethflow_addresses(&self) -> Vec<Address>;

    /// Queries the on-chain status of an order by its hash.
    ///
    /// The `ethflow_address` specifies which EthFlow contract to query.
    async fn get_order_status(
        &self,
        ethflow_address: Address,
        order_hash: B256,
    ) -> Result<RefundStatus>;
}

/// Abstracts blockchain write operations (transaction submission).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ChainWrite: Send + Sync {
    /// Submits a batch refund transaction to the EthFlow contract.
    ///
    /// Takes ownership of `encoded_ethflow_orders` because the contract binding
    /// requires it, so this avoids unnecessary cloning that would happen if we
    /// accepted a slice instead.
    async fn submit_refund(
        &mut self,
        uids: &[OrderUid],
        encoded_ethflow_orders: Vec<EthFlowOrder::Data>,
        ethflow_contract: Address,
    ) -> Result<()>;
}
