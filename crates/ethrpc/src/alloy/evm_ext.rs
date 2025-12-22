//! This module contains extensions specific to `evm_` RPC calls that alloy does
//! not provide (even though `anvil_` versions may exist).

use alloy::{
    primitives::BlockTimestamp,
    providers::Provider,
    transports::{RpcError, TransportErrorKind},
};

/// Transport extensions based on the `evm_` namespace.
pub trait EvmProviderExt {
    /// Set the automatic mining of new blocks with each new transaction
    /// submitted to the network.
    fn evm_set_automine(
        &self,
        automine: bool,
    ) -> impl std::future::Future<Output = Result<(), RpcError<TransportErrorKind>>> + Send;

    /// Sets the timestamp for the next block and increases the time
    /// accordingly.
    fn evm_set_next_block_timestamp(
        &self,
        timestamp: BlockTimestamp,
    ) -> impl std::future::Future<Output = Result<(), RpcError<TransportErrorKind>>> + Send;

    /// Enables (if `interval != 0`) or disables (if `interval == 0`) automatic
    /// block mining with the given `interval` of milliseconds.
    fn evm_set_interval_mining(
        &self,
        interval_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RpcError<TransportErrorKind>>> + Send;

    /// Sets the block gas limit for the following blocks.
    fn evm_set_block_gas_limit(
        &self,
        gas_limit: u64,
    ) -> impl std::future::Future<Output = Result<bool, RpcError<TransportErrorKind>>> + Send;
}

impl<T: Provider> EvmProviderExt for T {
    async fn evm_set_automine(&self, automine: bool) -> Result<(), RpcError<TransportErrorKind>> {
        self.raw_request("evm_setAutomine".into(), (automine,))
            .await
    }

    async fn evm_set_next_block_timestamp(
        &self,
        timestamp: BlockTimestamp,
    ) -> Result<(), RpcError<TransportErrorKind>> {
        self.raw_request("evm_setNextBlockTimestamp".into(), (timestamp,))
            .await
    }

    async fn evm_set_interval_mining(
        &self,
        interval_ms: u64,
    ) -> Result<(), RpcError<TransportErrorKind>> {
        self.raw_request("evm_setIntervalMining".into(), (interval_ms,))
            .await
    }

    async fn evm_set_block_gas_limit(
        &self,
        gas_limit: u64,
    ) -> Result<bool, RpcError<TransportErrorKind>> {
        self.raw_request("evm_setBlockGasLimit".into(), (gas_limit,))
            .await
    }
}
