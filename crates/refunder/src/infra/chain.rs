//! Blockchain access via Alloy.

use {
    crate::traits::{ChainRead, RefundStatus},
    alloy::{primitives::Address, providers::Provider, rpc::types::TransactionRequest},
    anyhow::{Result, anyhow},
    contracts::alloy::CoWSwapEthFlow,
    ethrpc::{AlloyProvider, block_stream::timestamp_of_current_block_in_seconds},
    std::collections::HashMap,
};

/// [`ChainRead`] implementation using Alloy.
pub struct AlloyChain {
    provider: AlloyProvider,
    ethflow_contracts: HashMap<Address, CoWSwapEthFlow::Instance>,
}

impl AlloyChain {
    pub fn new(provider: AlloyProvider, ethflow_addresses: Vec<Address>) -> Self {
        let ethflow_contracts = ethflow_addresses
            .into_iter()
            .map(|addr| {
                let instance = CoWSwapEthFlow::Instance::new(addr, provider.clone());
                (addr, instance)
            })
            .collect();
        Self {
            provider,
            ethflow_contracts,
        }
    }
}

impl ChainRead for AlloyChain {
    async fn current_block_timestamp(&self) -> Result<u32> {
        timestamp_of_current_block_in_seconds(&self.provider).await
    }

    async fn can_receive_eth(&self, address: Address) -> bool {
        let tx = TransactionRequest::default()
            .to(address)
            .value(alloy::primitives::U256::from(1));

        self.provider.estimate_gas(tx).await.is_ok()
    }

    fn ethflow_addresses(&self) -> Vec<Address> {
        self.ethflow_contracts.keys().copied().collect()
    }

    async fn get_order_status(
        &self,
        ethflow_address: Address,
        order_hash: alloy::primitives::B256,
    ) -> Result<RefundStatus> {
        let contract = self
            .ethflow_contracts
            .get(&ethflow_address)
            .ok_or_else(|| anyhow!("Unknown EthFlow contract: {ethflow_address}"))?;

        let order = contract.orders(order_hash).call().await?;
        Ok(order.into())
    }
}
