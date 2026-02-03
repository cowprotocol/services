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

    /// Returns `true` if `address` can receive ETH
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

#[cfg(test)]
pub mod test {
    use super::*;

    /// Extension trait for `MockChainRead` to reduce mock setup boilerplate.
    pub trait MockChainReadExt {
        fn with_block_timestamp(&mut self, timestamp: u32) -> &mut Self;
        fn with_ethflow_addresses(&mut self, addresses: Vec<Address>) -> &mut Self;
        fn with_order_status(&mut self, status: RefundStatus) -> &mut Self;
        fn receiving_eth(&mut self) -> &mut Self;
    }

    impl MockChainReadExt for MockChainRead {
        fn with_block_timestamp(&mut self, timestamp: u32) -> &mut Self {
            self.expect_current_block_timestamp()
                .returning(move || Ok(timestamp));
            self
        }

        fn with_ethflow_addresses(&mut self, addresses: Vec<Address>) -> &mut Self {
            self.expect_ethflow_addresses()
                .returning(move || addresses.clone());
            self
        }

        fn with_order_status(&mut self, status: RefundStatus) -> &mut Self {
            self.expect_get_order_status()
                .returning(move |_, _| Ok(status));
            self
        }

        fn receiving_eth(&mut self) -> &mut Self {
            self.expect_can_receive_eth().returning(|_| true);
            self
        }
    }

    /// Extension trait for `MockDbRead` to reduce mock setup boilerplate.
    pub trait MockDbReadExt {
        fn with_default_ethflow_order_data(&mut self) -> &mut Self;
        fn with_refundable_orders(&mut self, orders: Vec<EthOrderPlacement>) -> &mut Self;
    }

    impl MockDbReadExt for MockDbRead {
        fn with_default_ethflow_order_data(&mut self) -> &mut Self {
            self.expect_get_ethflow_order_data()
                .returning(|_| Ok(EthFlowOrder::Data::default()));
            self
        }

        fn with_refundable_orders(&mut self, orders: Vec<EthOrderPlacement>) -> &mut Self {
            self.expect_get_refundable_orders()
                .returning(move |_, _, _| Ok(orders.clone()));
            self
        }
    }
}
